// UI regression test: RAW tiles in the loupe (ADR 0006 step 4) against a mocked backend.
// Needs Node + Playwright (not part of the app build):  node ui/tests/raw-tiles.test.js
const { chromium } = require('playwright');
const fs = require('fs'); const path = require('path');
const WEB = path.join(__dirname, '..', 'web');
const FW = 3200, FH = 2400, T = 1024; // full RAW size of the mocked files, tile size
const svg = (w, h, c) => `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">` +
  `<rect width="${w}" height="${h}" fill="${c}"/></svg>`;
const mock = `
window.__calls = []; window.__h = {};
const files = [{index:0,name:'TN100423.ARW',size:1,ratable:true},{index:1,name:'TN005956.ARW',size:1,ratable:true},
  {index:2,name:'TN100426.ARW',size:1,ratable:true},{index:3,name:'P1240231.RW2',size:1,ratable:true}];
const med = {width:400,height:300,bytes:1,source:'m'};
window.__TAURI__ = { core: { invoke: async (cmd, a) => {
  window.__calls.push([cmd, JSON.parse(JSON.stringify(a || {}))]);
  switch (cmd) {
    case 'initial_folder': return 'D:/x';
    case 'open_folder': return {generation:1, folder:'D:/x', files};
    case 'read_ratings': return [];
    case 'image_info': return {name:files[a.index].name, orientation: a.index === 2 ? 6 : 1, previews:[med],
      small:med, medium:med, large: a.index === 1 ? {width:${FW},height:${FH},bytes:1,source:'l'} : med,
      width:${FW}, height:${FH}};
    case 'raw_view': return {enabled:true, want_half:false, can_tile:true, grid:null};
    default: return null;
  }}}, event: { listen: async (n, cb) => { window.__h[n] = cb; } } };
window.PW_BASE = 'http://pw.test/';
`;
// Screen point -> position in the image (0..1), from the loupe image's own transform.
const probe = (x, y) => {
  const img = document.getElementById('loupe-img');
  const vr = document.getElementById('viewport').getBoundingClientRect();
  const w = parseFloat(img.style.width), h = parseFloat(img.style.height);
  const m = new DOMMatrix(getComputedStyle(img).transform);
  const q = m.inverse().transformPoint(new DOMPoint(x - vr.left - w / 2, y - vr.top - h / 2));
  return [(q.x + w / 2) / w, (q.y + h / 2) / h];
};
(async () => {
  const ok = (c, m) => { if (!c) { console.log('FAIL', m); process.exitCode = 1; } else console.log('ok  ', m); };
  const b = await chromium.launch(); const p = await b.newPage({viewport:{width:1200,height:800}});
  const errors = []; p.on('pageerror', e => errors.push(String(e)));
  await p.route('http://app.test/**', r => {
    const f = path.join(WEB, new URL(r.request().url()).pathname.slice(1) || 'index.html');
    let body = fs.readFileSync(f, 'utf8');
    if (f.endsWith('index.html')) body = body.replace('<script src="app.js">', `<script>${mock}</script><script src="app.js">`);
    r.fulfill({ body, contentType: f.endsWith('.js') ? 'text/javascript' : f.endsWith('.css') ? 'text/css' : 'text/html' });
  });
  await p.route('http://pw.test/**', r => {
    const u = r.request().url();
    const m = u.match(/\/tile\/(\d+)\/(\d+)$/);
    if (m) {
      const c = +m[1], rr = +m[2];
      const w = Math.min(T, FW - c * T), h = Math.min(T, FH - rr * T);
      return r.fulfill({ body: svg(w, h, `rgb(${40 + c * 50},${40 + rr * 60},200)`), contentType: 'image/svg+xml', headers: {'Access-Control-Allow-Origin':'*'} });
    }
    const large = u.endsWith('/1/large');
    r.fulfill({ body: large ? svg(FW, FH, '#655') : svg(400, 300, '#468'), contentType: 'image/svg+xml', headers: {'Access-Control-Allow-Origin':'*'} });
  });
  await p.goto('http://app.test/index.html');
  await p.evaluate(() => localStorage.clear());
  await p.waitForTimeout(300);
  const calls = (c) => p.evaluate((c) => window.__calls.filter(x => x[0] === c).map(x => x[1]), c);
  const clear = () => p.evaluate(() => { window.__calls.length = 0; });
  const emit = (n, payload) => p.evaluate(([n, payload]) => window.__h[n]({ payload }), [n, payload]);
  const tilesShown = () => p.$$eval('#loupe-tiles img', a => a.length);
  const st = () => p.evaluate(() => rawTilesState());
  const vp = await p.$eval('#viewport', e => { const r = e.getBoundingClientRect(); return {x: r.left, y: r.top, w: r.width, h: r.height}; });
  const at = (x, y) => p.evaluate(`(${probe})(${x}, ${y})`);
  // Every shown tile sits exactly where its pixels belong in the image (centre within 0.5 px).
  const geometry = async (label) => {
    const boxes = await p.$$eval('#loupe-tiles img', a => a.map(i => {
      const r = i.getBoundingClientRect();
      const m = i.src.match(/tile\/(\d+)\/(\d+)$/);
      return { c: +m[1], r: +m[2], x: r.left + r.width / 2, y: r.top + r.height / 2, w: r.width, h: r.height };
    }));
    let worst = 0;
    for (const t of boxes) {
      const [u, v] = await at(t.x, t.y);
      const tw = Math.min(T, FW - t.c * T), th = Math.min(T, FH - t.r * T);
      worst = Math.max(worst, Math.abs(u * FW - (t.c * T + tw / 2)), Math.abs(v * FH - (t.r * T + th / 2)));
    }
    ok(boxes.length > 0 && worst < 0.5, `${label}: ${boxes.length} tiles placed on their image pixels (worst ${worst.toFixed(3)} px)`);
    // Neighbouring tiles meet edge to edge: no gap where the image below shows, no overlap.
    let seam = 0, pairs = 0;
    for (const t of boxes) for (const n of boxes) {
      if (!((n.r === t.r && n.c === t.c + 1) || (n.c === t.c && n.r === t.r + 1))) continue;
      const sepX = Math.max(t.x - t.w / 2, n.x - n.w / 2) - Math.min(t.x + t.w / 2, n.x + n.w / 2);
      const sepY = Math.max(t.y - t.h / 2, n.y - n.h / 2) - Math.min(t.y + t.h / 2, n.y + n.h / 2);
      seam = Math.max(seam, Math.abs(Math.max(sepX, sepY)));
      pairs++;
    }
    if (pairs) ok(seam < 0.05, `${label}: ${pairs} neighbour pairs meet edge to edge (worst ${seam.toFixed(3)} px)`);
    return boxes;
  };
  const tileUnder = async (x, y) => {
    const [u, v] = await at(x, y);
    return [Math.floor(u * FW / T), Math.floor(v * FH / T)];
  };
  const deliver = async (list) => {
    for (const [c, r] of list) await emit('raw-tile', { generation: 1, index: await p.evaluate(() => view.index), col: c, row: r });
    await p.waitForTimeout(400);
  };

  await p.keyboard.press('Enter');
  await p.waitForTimeout(400);
  ok((await calls('raw_prepare')).length === 0 && (await calls('raw_tiles')).length === 0, 'at fit: no unpack, no tiles');

  // First zoom action: unpack requested at once; tiles need the grid (comes with the unpack).
  const mx = vp.x + 380, my = vp.y + 260;
  await p.mouse.move(mx, my);
  for (let i = 0; i < 5; i++) { await p.mouse.wheel(0, -300); await p.waitForTimeout(30); }
  await p.waitForTimeout(100);
  ok((await calls('raw_prepare')).length === 1, 'first zoom action unpacks (raw_prepare once)');
  ok((await calls('raw_tiles')).length === 0, 'no tile request before the grid is known');
  const srcBefore = await p.$eval('#loupe-img', i => i.getAttribute('src'));
  await emit('raw-grid', { generation: 1, index: 0, width: FW, height: FH, tile: T });
  await p.waitForTimeout(100);
  let req = (await calls('raw_tiles')).pop();
  ok(req && req.tiles.length > 0, `tiles requested after the grid: ${JSON.stringify(req && req.tiles)}`);
  const under = await tileUnder(mx, my);
  ok(req && JSON.stringify(req.tiles[0]) === JSON.stringify(under), `tile under the pointer first ${JSON.stringify(under)}`);
  ok(!(await st()).covered, 'not covered yet');
  ok(await tilesShown() === 0 && (await p.$eval('#loupe-img', i => i.getAttribute('src'))) === srcBefore,
    'image stays as it is until tiles are decoded');

  // Tiles arrive: shown over the image, exactly in place; coverage reported.
  const covered = p.evaluate(() => new Promise(r => document.addEventListener('pw-tiles-covered', e => r(e.detail), { once: true })));
  await deliver(req.tiles);
  ok(await tilesShown() === req.tiles.length, `${req.tiles.length} tiles shown`);
  ok((await covered).tiles === req.tiles.length && (await st()).covered, 'visible area covered (event)');
  await geometry('zoomed');
  ok((await p.textContent('#loupe-badge')).includes('RAW-rutor'), 'badge mentions RAW tiles');

  // Pan: tiles follow the image; only new tiles are requested, nearest the pointer first.
  await clear();
  await p.mouse.move(vp.x + 300, vp.y + 300);
  await p.mouse.down(); await p.mouse.move(vp.x + 100, vp.y + 80, { steps: 8 }); await p.mouse.up();
  await p.waitForTimeout(100);
  await geometry('after pan');
  req = (await calls('raw_tiles')).pop();
  const shownKeys = await p.$$eval('#loupe-tiles img', a => a.map(i => i.src.match(/tile\/(\d+\/\d+)$/)[1].replace('/', ',')));
  ok(req && req.tiles.length > 0 && req.tiles.every(t => !shownKeys.includes(t.join(','))),
    `only missing tiles requested after the pan: ${JSON.stringify(req && req.tiles)} (shown ${shownKeys})`);
  if (req && req.tiles.length) await deliver(req.tiles);
  ok((await st()).covered, 'covered again after the pan');

  // 100 % = the full RAW (3200 over a 400 px preview = 8x).
  await p.evaluate(() => { fitToWindow(); zoomTo100(); });
  ok(Math.abs(await p.evaluate(() => view.scale) - 8) < 1e-6, '100 % = full RAW size');
  ok((await p.textContent('#status-zoom')).includes('100 %'), 'zoom status 100 %');
  req = (await calls('raw_tiles')).pop();
  if (req && req.tiles.length) await deliver(req.tiles);
  await geometry('100 %');

  // Rapid zoom and pan: no errors, requests only when the needed set changes, newest first.
  await clear();
  for (let i = 0; i < 40; i++) {
    const x = vp.x + 200 + (i * 97) % 800, y = vp.y + 150 + (i * 53) % 450;
    await p.mouse.move(x, y);
    if (i % 3) await p.mouse.wheel(0, i % 2 ? -200 : 180);
    else { await p.mouse.down(); await p.mouse.move(x + 60, y - 40, { steps: 2 }); await p.mouse.up(); }
  }
  await p.waitForTimeout(150);
  const n = (await calls('raw_tiles')).length;
  ok(n > 0 && n <= 60, `rapid interaction: ${n} tile requests for 40 actions`);
  const s1 = await st();
  req = (await calls('raw_tiles')).pop();
  if (s1.need && req && req.tiles.length) {
    const lastMouse = await p.evaluate(() => mouseAt);
    const u2 = await tileUnder(vp.x + lastMouse[0], vp.y + lastMouse[1]);
    ok(JSON.stringify(req.tiles[0]) === JSON.stringify(u2) || (await p.$$eval('#loupe-tiles img', a => a.map(i => i.src))).some(s => s.endsWith(`tile/${u2[0]}/${u2[1]}`)),
      'after rapid interaction the tile under the pointer is first (or already shown)');
    await deliver(req.tiles);
  }
  await geometry('after rapid interaction');

  // Next image: tiles disappear with the old image at once.
  await p.keyboard.press('ArrowRight');
  await p.waitForTimeout(300);
  ok(await tilesShown() === 0, 'tiles of the previous image removed');

  // A1-like file: full-size embedded JPEG is enough -> no unpack, no tiles, M1 100 %.
  await clear();
  await p.mouse.move(vp.x + 500, vp.y + 300);
  for (let i = 0; i < 6; i++) { await p.mouse.wheel(0, -300); await p.waitForTimeout(30); }
  await p.evaluate(() => { fitToWindow(); zoomTo100(); });
  await p.waitForTimeout(500);
  ok((await calls('raw_prepare')).length === 0 && (await calls('raw_tiles')).length === 0, 'A1: no unpack and no tiles');
  ok(await p.evaluate(() => view.level) === 'large', 'A1: the embedded full-size JPEG is used (M1)');

  // Rotated file (orientation 6): tiles in sensor orientation sit correctly.
  await p.keyboard.press('ArrowRight');
  await p.waitForTimeout(300);
  await p.mouse.move(vp.x + 450, vp.y + 350);
  for (let i = 0; i < 5; i++) { await p.mouse.wheel(0, -300); await p.waitForTimeout(30); }
  await emit('raw-grid', { generation: 1, index: 2, width: FW, height: FH, tile: T });
  await p.waitForTimeout(100);
  req = (await calls('raw_tiles')).pop();
  ok(req && req.index === 2 && JSON.stringify(req.tiles[0]) === JSON.stringify(await tileUnder(vp.x + 450, vp.y + 350)), 'rotated: tile under the pointer first');
  await deliver(req.tiles);
  await geometry('rotated 90°');

  // RAW rendering off: tiles go, nothing is requested any more (M1).
  await p.click('#status-raw');
  await p.waitForTimeout(200);
  ok(await tilesShown() === 0, 'RAW off: tiles removed');
  await clear();
  for (let i = 0; i < 3; i++) { await p.mouse.wheel(0, -300); await p.waitForTimeout(30); }
  ok((await calls('raw_prepare')).length === 0 && (await calls('raw_tiles')).length === 0, 'RAW off: no RAW work');
  await p.click('#status-raw');

  ok(errors.length === 0, 'no page errors ' + errors.join(' | '));
  await b.close();
})();
