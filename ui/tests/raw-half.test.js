// UI regression test: half-size RAW in the loupe (ADR 0006 step 3) against a mocked backend.
// Needs Node + Playwright (not part of the app build):  node ui/tests/raw-half.test.js
const { chromium } = require('playwright');
const fs = require('fs'); const path = require('path');
const WEB = path.join(__dirname, '..', 'web');
const svg = (w, h, c) => `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">` +
  `<rect width="${w}" height="${h}" fill="${c}"/><circle cx="${w / 3}" cy="${h / 3}" r="${w / 10}" fill="#fc0"/></svg>`;
const mock = `
window.__calls = []; window.__h = {};
const files = [{index:0,name:'TN100423.ARW',size:1,ratable:true},{index:1,name:'TN005956.ARW',size:1,ratable:true},{index:2,name:'P1240231.RW2',size:1,ratable:true}];
const med = {width:400,height:300,bytes:1,source:'m'};
window.__TAURI__ = { core: { invoke: async (cmd, a) => {
  window.__calls.push([cmd, JSON.parse(JSON.stringify(a || {}))]);
  switch (cmd) {
    case 'initial_folder': return 'D:/x';
    case 'open_folder': return {generation:1, folder:'D:/x', files};
    case 'read_ratings': return [];
    case 'image_info': return {name:files[a.index].name, orientation:1, previews:[med], small:med, medium:med, large:med, width:800, height:600};
    case 'raw_view': return {enabled:true, want_half: a.index === 0 || a.index === 2};
    case 'raw_set_enabled': return null;
  }}}, event: { listen: async (n, cb) => { window.__h[n] = cb; } } };
window.PW_BASE = 'http://pw.test/';
`;
const probe = (x, y) => {
  const img = document.getElementById('loupe-img');
  const w = parseFloat(img.style.width), h = parseFloat(img.style.height);
  const m = new DOMMatrix(getComputedStyle(img).transform);
  const q = m.inverse().transformPoint(new DOMPoint(x - w / 2, y - h / 2));
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
    const half = r.request().url().endsWith('/rawhalf');
    r.fulfill({ body: half ? svg(800, 600, '#486') : svg(400, 300, '#468'), contentType: 'image/svg+xml', headers: {'Access-Control-Allow-Origin':'*'} });
  });
  await p.goto('http://app.test/index.html');
  await p.evaluate(() => localStorage.clear());
  await p.waitForTimeout(300);
  const calls = (c) => p.evaluate((c) => window.__calls.filter(x => x[0] === c).map(x => x[1]), c);
  const emit = (n, payload) => p.evaluate(([n, payload]) => window.__h[n]({ payload }), [n, payload]);
  const src = () => p.$eval('#loupe-img', i => i.getAttribute('src'));
  const vp = await p.$eval('#viewport', e => { const r = e.getBoundingClientRect(); return {w: r.width, h: r.height}; });
  const at = (x, y) => p.evaluate(`(${probe})(${x}, ${y})`);

  ok((await p.textContent('#status-raw')).includes('på'), 'setting shown, on by default');
  ok((await calls('raw_set_enabled')).some(a => a.enabled === true && a.generation === 1), 'backend told the setting for the open folder');
  ok((await calls('raw_view')).length === 0, 'contact sheet starts no RAW work');

  await p.keyboard.press('Enter');
  await p.waitForTimeout(300);
  const rv = (await calls('raw_view')).pop();
  ok(rv && rv.index === 0 && rv.screenEdge > 0 && JSON.stringify(rv.neighbours) === '[1]', 'loupe asks for RAW: ' + JSON.stringify(rv));
  ok((await src()).endsWith('/medium'), 'embedded preview (M1) shown first');

  // Zoom in off-centre, then the half-size image arrives: framing must not jump.
  await p.mouse.move(400, 300);
  for (let i = 0; i < 4; i++) { await p.mouse.wheel(0, -300); await p.waitForTimeout(30); }
  const before = await at(300, 250);
  const box0 = await p.$eval('#loupe-img', i => i.getBoundingClientRect().width);
  await emit('raw-ready', { generation: 1, index: 0, kind: 'half', full_width: 3200, full_height: 2400 });
  await p.waitForTimeout(400);
  ok((await src()).endsWith('/rawhalf'), 'half-size RAW replaces the preview');
  const after = await at(300, 250);
  const box1 = await p.$eval('#loupe-img', i => i.getBoundingClientRect().width);
  ok(Math.abs(before[0] - after[0]) < 0.003 && Math.abs(before[1] - after[1]) < 0.003, `same point under the cursor ${before} -> ${after}`);
  ok(Math.abs(box0 - box1) < 1, `same on-screen size ${box0} -> ${box1}`);
  ok((await p.textContent('#loupe-badge')).includes('RAW-avkodning'), 'badge says RAW');
  ok((await p.textContent('#status-zoom')).includes('%'), 'zoom status');
  ok((await p.textContent('#loupe-badge')).includes('3200'), 'badge gives the full RAW width');
  // The half-size image is scaled to the screen: 100 % means the full RAW size reported by the
  // backend (3200 px over an 800 px image = 4x), not twice the half-size image.
  await p.evaluate(() => { fitToWindow(); zoomTo100(); });
  ok(Math.abs(await p.evaluate(() => view.scale) - 4) < 1e-6, '100 % = full RAW size');
  ok((await p.textContent('#status-zoom')).includes('100 %'), 'zoom status says 100 %: ' + await p.textContent('#status-zoom'));
  await p.evaluate(() => fitToWindow());

  // A1: the embedded preview is enough (backend says no) -> a raw-ready never swaps.
  await p.keyboard.press('ArrowRight');
  await p.waitForTimeout(300);
  await emit('raw-ready', { generation: 1, index: 1, kind: 'half' });
  await p.waitForTimeout(300);
  ok((await src()).endsWith('/medium'), 'A1 keeps the embedded preview');

  // RW2: decoding fails -> preview stays, badge explains.
  await p.keyboard.press('ArrowRight');
  await p.waitForTimeout(300);
  await emit('raw-failed', { generation: 1, index: 2, kind: 'failed', message: 'Unsupported file format' });
  await p.waitForTimeout(200);
  ok((await src()).endsWith('/medium'), 'failed RAW keeps the M1 preview');
  ok((await p.textContent('#loupe-badge')).includes('kunde inte avkodas'), 'badge reports the failure');

  // Stale event for another folder generation is ignored.
  await emit('raw-ready', { generation: 99, index: 2, kind: 'half' });
  await p.waitForTimeout(200);
  ok((await src()).endsWith('/medium'), 'event of an old folder ignored');

  // Back to the contact sheet: RAW work is released.
  await p.keyboard.press('Escape');
  await p.waitForTimeout(200);
  ok((await calls('raw_view')).pop().index === null, 'contact sheet clears the current RAW image');

  // Setting off: back to M1 only.
  await p.keyboard.press('Home');
  await p.keyboard.press('Enter');
  await p.waitForTimeout(300);
  await emit('raw-ready', { generation: 1, index: 0, kind: 'half' });
  await p.waitForTimeout(300);
  ok((await src()).endsWith('/rawhalf'), 'RAW shown again for image 0');
  await p.click('#status-raw');
  await p.waitForTimeout(300);
  ok((await p.textContent('#status-raw')).includes('av'), 'setting off');
  ok((await calls('raw_set_enabled')).pop().enabled === false, 'backend told off');
  ok((await src()).endsWith('/medium'), 'off returns to the embedded preview');
  ok(await p.evaluate(() => localStorage.getItem('rawRendering')) === 'off', 'choice remembered');
  const n = (await calls('raw_view')).length;
  await p.keyboard.press('ArrowRight'); await p.keyboard.press('ArrowLeft');
  await p.waitForTimeout(300);
  ok((await calls('raw_view')).length === n, 'no RAW requests while off');
  await emit('raw-ready', { generation: 1, index: 0, kind: 'half' });
  await p.waitForTimeout(200);
  ok((await src()).endsWith('/medium'), 'events ignored while off');
  await p.click('#status-raw');
  await p.waitForTimeout(300);
  ok((await calls('raw_view')).length > n, 'on again asks for the current image');

  ok(errors.length === 0, 'no page errors ' + errors.join('|'));
  await b.close();
})();
