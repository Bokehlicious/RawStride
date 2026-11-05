// PhotoWorkflow – built-in measurement run (ADR 0006, step 3). Only active when the app is
// started with `--bench <out.json>` (see src-tauri/src/bench.rs); otherwise it does nothing.
// The same scenario runs with RAW rendering on and off (off = the M1 code paths).
"use strict";

(async () => {
  let cfg = null;
  try { cfg = await invoke("bench_config"); } catch (_) { return; }
  if (!cfg) return;

  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  // A window behind other windows gets no animation frames (WebView2 occlusion): fall back
  // after 250 ms so the run never stalls, and count it (frame times are then not meaningful).
  let frameFallbacks = 0;
  const frame = () => new Promise((r) => {
    let done = false;
    requestAnimationFrame(() => { if (!done) { done = true; r(); } });
    setTimeout(() => { if (!done) { done = true; frameFallbacks++; r(); } }, 250);
  });
  const now = () => performance.now();
  const epoch = () => Math.round(performance.timeOrigin + performance.now());
  const name = (i) => (state.files[i] || {}).name;
  const out = {
    config: cfg, userAgent: navigator.userAgent, screenEdge: rawScreenEdge(),
    devicePixelRatio: window.devicePixelRatio, viewport: [innerWidth, innerHeight],
    phases: [], grid: [], loupe: [], revisit: [], flick: {}, zoom: {}, cold: [], tiles: [], failure: [], gridErrors: [], decode: [], stats: [], errors: [], toastAfterOpen: null,
  };
  // Each phase also writes what is there so far: a run that stops shows where.
  window.__benchOut = out;
  const phase = (p) => {
    out.phases.push({ phase: p, epochMs: epoch() });
    invoke("bench_write", { json: JSON.stringify(out, null, 1) }).catch(() => {});
  };
  // Script errors in the page (step 5: the standalone build must run without any).
  window.addEventListener("error", (e) => out.errors.push(String(e.message || e)));
  window.addEventListener("unhandledrejection", (e) => out.errors.push(String(e.reason)));

  /// Resolves with the event's detail when `pred` matches, or null after `ms`.
  function waitEvent(type, pred, ms) {
    return new Promise((resolve) => {
      const on = (e) => { if (pred(e.detail)) { done(); resolve(e.detail); } };
      const t = setTimeout(() => { done(); resolve(null); }, ms);
      function done() { clearTimeout(t); document.removeEventListener(type, on); }
      document.addEventListener(type, on);
    });
  }
  async function stats(label) {
    try { out.stats.push({ label, epochMs: epoch(), ...(await invoke("raw_stats")) }); } catch (_) {}
  }
  /// Whether the backend wants a half-size image for `index` (answer to raw_view), or false.
  async function wanted(index) {
    if (!raw.enabled) return false;
    if (raw.wanted.has(index)) return raw.wanted.get(index);
    const d = await waitEvent("pw-raw-wanted", (x) => x.index === index, 1500);
    return !!(d && d.want);
  }
  /// Shows `index` in the loupe (via `go` or directly) and times the preview and the RAW image.
  async function showAndTime(index, move) {
    const t0 = now();
    const shown = waitEvent("pw-shown", (d) => d.index === index && d.level !== "rawhalf", 10000);
    const rawShown = waitEvent("pw-raw-shown", (d) => d.index === index, 20000);
    const rawFailed = waitEvent("pw-raw-failed", (d) => d.index === index, 20000);
    move();
    const s = await shown;
    const previewMs = s ? now() - t0 : null;
    const want = await wanted(index);
    let rawMs = null, decodeMs = null, failed = null;
    if (want) {
      const r = await Promise.race([rawShown, rawFailed.then((f) => (f ? { failed: f } : null))]);
      if (r && r.failed) failed = r.failed.message;
      else if (r) { rawMs = now() - t0; decodeMs = r.decodeMs; }
    }
    return { index, name: name(index), previewMs, wantHalf: want, rawMs, rawDecodeMs: decodeMs, failed };
  }

  rawSetEnabled(!!cfg.raw, false);
  if (cfg.half_max != null) await invoke("raw_set_half_max", { maxEdge: cfg.half_max, generation: state.generation });
  phase("open");
  await openFolder(cfg.folder);
  await sleep(2500); // thumbnails, ExifTool warm-up
  out.toastAfterOpen = el.toast.hidden ? null : el.toast.textContent;
  const n = state.visible.length;

  // 1. Contact sheet: move the selection, time until the info panel has the new image.
  phase("grid");
  setMode("grid");
  select(state.visible[0]);
  await sleep(500);
  for (let k = 0; k < Math.min(n - 1, 20); k++) {
    const t0 = now();
    go(1);
    const idx = state.current;
    // A file the backend cannot read (broken-file folder) rejects here; the app shows a message.
    try { await getInfo(idx); } catch (e) { out.gridErrors.push({ index: idx, name: name(idx), error: String(e) }); }
    await frame();
    out.grid.push({ index: idx, ms: now() - t0 });
    await sleep(250);
  }
  await stats("after-grid");

  // 2. Loupe: every image once (first visit), with time to look at it.
  phase("loupe");
  select(state.visible[0]);
  out.loupe.push(await showAndTime(state.visible[0], () => setMode("loupe")));
  await sleep(1200);
  for (let p = 1; p < n; p++) {
    out.loupe.push(await showAndTime(state.visible[p], () => go(1)));
    await stats(`loupe-${p}`);
    await sleep(1200);
  }

  // 3. Revisit: back through the images (results come from the caches when still there).
  phase("revisit");
  for (let p = n - 2; p >= Math.max(0, n - 6); p--) {
    out.revisit.push(await showAndTime(state.visible[p], () => go(-1)));
    await sleep(800);
  }
  await stats("after-revisit");

  // 4. Fast browsing: 60 steps at 40 ms; event-loop lag shows whether the UI keeps up.
  phase("flick");
  go(-n);
  await sleep(1500);
  const lags = [];
  for (let k = 0; k < 60; k++) {
    const due = now() + 40;
    await sleep(40);
    lags.push(now() - due);
    go(k % (2 * n) < n ? 1 : -1);
  }
  const last = state.current;
  const tStop = now();
  const settled = await showAndTime(last, () => {});
  out.flick = {
    steps: 60, intervalMs: 40,
    lagAvgMs: lags.reduce((a, b) => a + b, 0) / lags.length,
    lagMaxMs: Math.max(...lags),
    finalIndex: last, finalName: name(last),
    finalPreviewAfterStopMs: settled.previewMs, finalRawAfterStopMs: settled.rawMs,
    stopToSettledMs: now() - tStop,
  };
  await stats("after-flick");

  // 5. Zoom and switch repeatedly (memory budget under load).
  phase("zoom");
  const zoomLag = [];
  for (let k = 0; k < 20; k++) {
    const t0 = now();
    zoomTo(view.scale * (k % 2 ? 0.5 : 2));
    await frame();
    zoomLag.push(now() - t0);
    if (k % 4 === 3) go(1);
    await sleep(150);
  }
  out.zoom = { steps: 20, frameAvgMs: zoomLag.reduce((a, b) => a + b, 0) / zoomLag.length, frameMaxMs: Math.max(...zoomLag) };
  await sleep(3000);
  await stats("after-zoom");

  // 6. First RAW image with nothing prepared: RAW caches emptied (setting off and on), then from
  //    the contact sheet straight into an A7R IVA image (cold), then the next one after a pause
  //    (prepared in the background as a neighbour). Twice, from different images.
  phase("cold");
  const a7rs = state.visible.filter((i) => /^TN1004/i.test(name(i) || ""));
  for (const start of [a7rs[2], a7rs[0]].filter((i) => i != null)) {
    setMode("grid");
    rawSetEnabled(false, false);
    rawSetEnabled(!!cfg.raw, false);
    await sleep(800);
    select(start);
    await sleep(300);
    const first = await showAndTime(start, () => setMode("loupe"));
    await sleep(1200);
    const next = state.visible[state.pos.get(start) + 1];
    const second = next != null ? await showAndTime(next, () => go(1)) : null;
    out.cold.push({ first, second });
    await stats("cold");
    await sleep(800);
  }

  // 7. RAW tiles (step 4): time to the first tile and until the visible area is covered, the wait
  //    for unpacking, panning and fast zoom/pan, per camera. With RAW off the same actions run on
  //    the M1 code paths (no tiles); their frame times are the comparison.
  const byName = (re) => state.visible.find((i) => re.test(name(i) || ""));
  const imgA7r = byName(/^TN100424/i), imgRw2 = byName(/^P1240233/i), imgA1 = byName(/^TN005960/i);
  const centre = () => { const r = el.viewport.getBoundingClientRect(); return [r.width / 2, r.height / 2]; };
  /// Fresh start on `index`: RAW caches emptied, loupe opened, preview shown, then `pauseMs`.
  async function enter(index, pauseMs, waitRaw = false) {
    setMode("grid");
    rawSetEnabled(false, false);
    rawSetEnabled(!!cfg.raw, false);
    await sleep(600);
    select(index);
    await sleep(200);
    const shown = waitEvent("pw-shown", (d) => d.index === index, 5000);
    const rawShown = waitEvent("pw-raw-shown", (d) => d.index === index, 8000);
    setMode("loupe");
    await shown;
    mouseAt = centre();
    if (waitRaw && cfg.raw && (await wanted(index))) await rawShown;
    await sleep(pauseMs);
  }
  /// Runs `action` and times the tiles that follow (ms from the action).
  async function tileAction(index, action, ms = 20000) {
    const t0 = now();
    let first = null, grid = null, covered = null;
    const onShown = (e) => { if (e.detail.index === index && first == null) first = now() - t0; };
    const onGrid = (e) => { if (e.detail.index === index && grid == null) grid = now() - t0; };
    const onCov = (e) => { if (e.detail.index === index && covered == null) covered = now() - t0; };
    document.addEventListener("pw-tile-shown", onShown);
    document.addEventListener("pw-raw-grid", onGrid);
    document.addEventListener("pw-tiles-covered", onCov);
    action();
    await frame();
    let st = rawTilesState();
    while (now() - t0 < ms) {
      st = rawTilesState();
      if (!st.need) break;
      if (st.covered && covered == null) covered = now() - t0;
      if (covered != null) break;
      await sleep(10);
    }
    document.removeEventListener("pw-tile-shown", onShown);
    document.removeEventListener("pw-raw-grid", onGrid);
    document.removeEventListener("pw-tiles-covered", onCov);
    return { firstTileMs: first, coveredMs: covered, gridMs: grid, need: st.need,
      visibleTiles: st.visible, shownTiles: st.shown };
  }
  /// Frame times (rAF deltas) while `during` runs.
  async function frames(during) {
    const ds = []; let run = true; let last = now();
    const loop = () => requestAnimationFrame(() => { const t = now(); ds.push(t - last); last = t; if (run) loop(); });
    loop();
    await during();
    run = false;
    await frame();
    return { frames: ds.length, avgMs: ds.reduce((a, b) => a + b, 0) / Math.max(1, ds.length), maxMs: Math.max(0, ...ds) };
  }
  /// Scale for `pct` of the full RAW size (or of the embedded image when there is no RAW ref).
  const scaleFor = (pct) => {
    const fw = rawTileRef(view.index) || (state.info.get(view.index).large || {}).width || view.w;
    return (pct * fw) / view.w;
  };
  const rec = (scenario, index, r, extra = {}) => out.tiles.push({ scenario, index, name: name(index), ...r, ...extra });

  if (imgA7r != null) {
    // a) Zoom to 100 % right away (half-size not ready: unpack in the interactive worker).
    phase("tiles-a7r-now");
    await enter(imgA7r, 150);
    rec("a7r-100-at-once", imgA7r, await tileAction(imgA7r, () => zoomTo100()));
    await stats("tiles-a7r-now");
    // b) Zoom to 100 % after looking at the image (half-size shown; its session is reused).
    phase("tiles-a7r-pause");
    await enter(imgA7r, 300, true);
    rec("a7r-100-after-pause", imgA7r, await tileAction(imgA7r, () => zoomTo100()));
    await stats("tiles-a7r-pause");
    // c) Zoom levels, each from a fresh start after a pause.
    phase("tiles-a7r-levels");
    // "tiles-start": 10 % past the zoom where the shown image runs out of detail (where tiles
    // begin; most of the image is visible there), "fit+30%": a small zoom from fit.
    for (const [label, pct] of [["fit+30%", "fit"], ["tiles-start", "start"], ["50%", 0.5], ["200%", 2]]) {
      await enter(imgA7r, 300, true);
      const info = state.info.get(view.index);
      const start = (rtAvailable(info) * 1.05) / (view.w * (window.devicePixelRatio || 1));
      const target = pct === "fit" ? view.fit * 1.3 : pct === "start" ? Math.max(start, view.fit * 1.02) * 1.1 : scaleFor(pct);
      rec(`a7r-${label}`, imgA7r, await tileAction(imgA7r, () => zoomTo(target)), { scale: target, fit: view.fit });
    }
    await stats("tiles-a7r-levels");
    // d) Panning at 100 %: 8 drags of 400 px (10 frames each), time to cover after each stop.
    phase("tiles-a7r-pan");
    await enter(imgA7r, 300, true);
    await tileAction(imgA7r, () => zoomTo100());
    for (let k = 0; k < 8; k++) {
      const dx = (k % 4 < 2 ? -1 : 1) * 40, dy = (k % 2 ? 1 : -1) * 15;
      const f = await frames(async () => { for (let i = 0; i < 10; i++) { view.px += dx; view.py += dy; render(); await frame(); } });
      rec("a7r-pan-100", imgA7r, await tileAction(imgA7r, () => {}), { pan: k, panFrameAvgMs: f.avgMs, panFrameMaxMs: f.maxMs });
    }
    await stats("tiles-a7r-pan");
    // e) Fast zoom and pan: 80 actions at 20 ms, then the time until the view is covered.
    phase("tiles-a7r-rapid");
    const lag = [];
    const f = await frames(async () => {
      for (let k = 0; k < 80; k++) {
        const due = now() + 20;
        await sleep(20);
        lag.push(now() - due);
        const [W, H] = centre().map((v) => v * 2);
        const px = (W * ((k * 37) % 100)) / 100, py = (H * ((k * 61) % 100)) / 100;
        mouseAt = [px, py];
        if (k % 3 === 2) { view.px += (k % 2 ? 1 : -1) * 150; view.py += (k % 4 < 2 ? 1 : -1) * 90; render(); }
        else {
          const want = view.scale * (k % 2 ? 1.25 : 0.82);
          zoomTo(Math.max(view.fit * 1.2, Math.min(want, scaleFor(2))), px, py);
        }
      }
    });
    rec("a7r-rapid", imgA7r, await tileAction(imgA7r, () => {}), {
      actions: 80, lagAvgMs: lag.reduce((a, b) => a + b, 0) / lag.length, lagMaxMs: Math.max(...lag),
      frameAvgMs: f.avgMs, frameMaxMs: f.maxMs,
    });
    await stats("tiles-a7r-rapid");
    // f) Repeated zoom in/out at different points (memory budget).
    phase("tiles-a7r-repeat");
    for (let k = 0; k < 10; k++) {
      fitToWindow();
      await frame();
      const [W, H] = centre().map((v) => v * 2);
      const px = (W * (15 + ((k * 29) % 70))) / 100, py = (H * (15 + ((k * 47) % 70))) / 100;
      mouseAt = [px, py];
      rec("a7r-repeat-100", imgA7r, await tileAction(imgA7r, () => zoomTo100(px, py)), { repeat: k });
      await stats(`tiles-repeat-${k}`);
    }
  }
  if (imgRw2 != null) {
    // g) RW2 (no half-size image on this screen): zoom to 100 % at once, and after a small first
    //    zoom action (which starts the unpack).
    phase("tiles-rw2");
    await enter(imgRw2, 400);
    rec("rw2-100-at-once", imgRw2, await tileAction(imgRw2, () => zoomTo100()));
    await enter(imgRw2, 400);
    zoomTo(view.fit * 1.1);
    await sleep(1000);
    rec("rw2-100-after-first-zoom", imgRw2, await tileAction(imgRw2, () => zoomTo100()));
    await stats("tiles-rw2");
  }
  if (imgA1 != null) {
    // h) A1: the embedded full-size JPEG is enough; no tiles, no unpack.
    phase("tiles-a1");
    await enter(imgA1, 400);
    const before = out.stats.length;
    await stats("tiles-a1-before");
    const r = await tileAction(imgA1, () => zoomTo100(), 3000);
    await sleep(1500);
    await stats("tiles-a1-after");
    rec("a1-100", imgA1, r, { level: view.level, shownAfter: rawTilesState().shown,
      sessionsOpened: out.stats.slice(before).map((x) => x.sessions_opened) });
  }
  setMode("grid");
  await sleep(500);

  // 8. Broken files (step 5): every file named *broken* is opened in the loupe and zoomed to
  //    100 %. The embedded preview (M1) must stay, the failure must be reported, and a good image
  //    must still work afterwards.
  const broken = state.visible.filter((i) => /broken/i.test(name(i) || ""));
  const warnText = () => { const w = document.getElementById("loupe-rawwarn"); return w && !w.hidden ? w.textContent : null; };
  if (broken.length) {
    phase("failure");
    for (const i of broken) {
      setMode("grid");
      await sleep(300);
      select(i);
      await sleep(200);
      el.toast.hidden = true;
      const shown = waitEvent("pw-shown", (d) => d.index === i, 4000);
      const failed = waitEvent("pw-raw-failed", (d) => d.index === i, 8000);
      setMode("loupe");
      const s = await shown;
      const want = await wanted(i);
      const f = want ? await failed : null;
      zoomTo100();
      await sleep(2500);
      out.failure.push({
        index: i, name: name(i), previewShown: !!s, level: view.level, wantHalf: want,
        failedOnHalf: f ? f.message : null, failedMessage: raw.failed.get(i) || null,
        tiles: rawTilesState(), src: el.img.getAttribute("src"), badge: el.badge.hidden ? null : el.badge.textContent,
        toast: el.toast.hidden ? null : el.toast.textContent, warn: warnText(),
      });
      await stats(`failure-${name(i)}`);
    }
    const good = byName(/^TN100424/i);
    if (good != null) {
      setMode("grid");
      await sleep(300);
      select(good);
      await sleep(200);
      const after = await showAndTime(good, () => setMode("loupe"));
      const z = await tileAction(good, () => zoomTo100());
      out.failure.push({ goodImageAfterwards: after, tilesAfterwards: z, warnOnGood: warnText() });
    }
    // Rapid switching over good, damaged and unreadable files while they are being decoded,
    // then a stop on a good, a damaged and an unreadable file.
    setMode("grid");
    rawSetEnabled(false, false);
    rawSetEnabled(true, false);
    await sleep(500);
    select(state.visible[0]);
    setMode("loupe");
    await sleep(300);
    for (let k = 0; k < 60; k++) {
      go(k % 9 < 6 ? 1 : -1);
      await sleep(40 + (k % 5) * 25);
    }
    const stops = [byName(/^TN100424/i), byName(/cut02/i), byName(/garbage/i), byName(/^P1240233/i)];
    const rapid = [];
    for (const i of stops.filter((x) => x != null)) {
      setMode("grid");
      await sleep(200);
      select(i);
      await sleep(100);
      const r = await showAndTime(i, () => setMode("loupe"));
      await sleep(1500);
      rapid.push({ index: i, name: name(i), viewIndex: view.index, level: view.level,
        src: el.img.getAttribute("src"), warn: warnText(), rawMs: r.rawMs, failed: raw.failed.get(i) || null,
        tilesOfOther: rt.index !== -1 && rt.index !== i, badge: el.badge.hidden ? null : el.badge.textContent });
    }
    out.failure.push({ rapidSwitching: rapid });
    setMode("grid");
    await sleep(300);
  }

  // 9. Half-size JPEG: render/scale/encode in Rust, then fetch and decode in WebView2,
  //    full half size versus scaled to the screen (only with RAW rendering on).
  if (cfg.raw) {
    phase("decode");
    const a7r = state.visible.find((i) => /^TN1004/i.test(name(i) || ""));
    if (a7r != null) {
      for (const maxEdge of [0, 3840, 2560, 1920]) {
        for (let rep = 0; rep < 3; rep++) {
          const b = await invoke("raw_bench_half", { generation: state.generation, index: a7r, maxEdge });
          const t0 = now();
          const resp = await fetch(`${PW}bench/half?${maxEdge}-${rep}-${Math.random()}`);
          const blob = await resp.blob();
          const fetchMs = now() - t0;
          const t1 = now();
          const bmp = await createImageBitmap(blob);
          const bitmapMs = now() - t1;
          bmp.close();
          const u = URL.createObjectURL(blob);
          const im = new Image();
          im.src = u;
          const t2 = now();
          await im.decode();
          const imgDecodeMs = now() - t2;
          URL.revokeObjectURL(u);
          out.decode.push({ maxEdge, rep, ...b, fetchMs, bitmapMs, imgDecodeMs });
          await sleep(300);
        }
      }
    }
  }

  out.frameFallbacks = frameFallbacks;
  out.visibility = document.visibilityState;
  phase("done");
  await stats("end");
  await invoke("bench_write", { json: JSON.stringify(out, null, 1) });
  await sleep(300);
  await invoke("bench_exit");
})().catch(async (e) => {
  // A measurement step failed: keep what was measured, say why, and end the run (never hang).
  const out = Object.assign({}, window.__benchOut || {}, { fatal: String((e && e.stack) || e) });
  try { await invoke("bench_write", { json: JSON.stringify(out, null, 1) }); } catch (_) {}
  invoke("bench_exit").catch(() => {});
});
