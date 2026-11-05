// UI regression test: zoom keeps the image point under the mouse cursor in place (wheel,
// double-click, Z and +/- keys), also for rotated images and across the switch to the large
// preview; with anchoring turned off, keys zoom around the centre.
// Needs Node + Playwright (not part of the app build):  node ui/tests/zoom-anchor.test.js
const { chromium } = require('playwright');
const fs = require('fs'); const path = require('path');
const WEB = path.join(__dirname, '..', 'web');
const svg = (w, h) => `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">` +
  `<rect width="${w}" height="${h}" fill="#468"/><circle cx="${w / 3}" cy="${h / 3}" r="${w / 10}" fill="#fc0"/></svg>`;
const mock = (orientation) => `
window.__calls = [];
const files = [{index:0,name:'A.ARW',size:1,ratable:true},{index:1,name:'B.ARW',size:1,ratable:true}];
const med = {width:400,height:300,bytes:1,source:'m'}, big = {width:1600,height:1200,bytes:1,source:'l'};
window.__TAURI__ = { core: { invoke: async (cmd, a) => {
  switch (cmd) {
    case 'initial_folder': return 'D:/x';
    case 'open_folder': return {generation:1, folder:'D:/x', files};
    case 'read_ratings': return [];
    case 'image_info': return {name:files[a.index].name, orientation:${orientation}, previews:[med,big], small:med, medium:med, large:big, width:1600, height:1200};
  }}}, event: { listen: async () => {} } };
window.PW_BASE = 'http://pw.test/';
`;

/// Image point (0..1, 0..1 in the unrotated image) under viewport point (x, y).
const probe = (x, y) => {
  const img = document.getElementById('loupe-img');
  const w = parseFloat(img.style.width), h = parseFloat(img.style.height);
  const m = new DOMMatrix(getComputedStyle(img).transform);
  const o = { x: w / 2, y: h / 2 };
  const q = m.inverse().transformPoint(new DOMPoint(x - o.x, y - o.y));
  return [(q.x + o.x) / w, (q.y + o.y) / h];
};

async function run(orientation, ok) {
  const b = await chromium.launch(); const p = await b.newPage({viewport:{width:1200,height:800}});
  const errors = []; p.on('pageerror', e => errors.push(String(e)));
  await p.route('http://app.test/**', r => {
    const f = path.join(WEB, new URL(r.request().url()).pathname.slice(1) || 'index.html');
    let body = fs.readFileSync(f, 'utf8');
    if (f.endsWith('index.html')) body = body.replace('<script src="app.js">', `<script>${mock(orientation)}</script><script src="app.js">`);
    r.fulfill({ body, contentType: f.endsWith('.js') ? 'text/javascript' : f.endsWith('.css') ? 'text/css' : 'text/html' });
  });
  await p.route('http://pw.test/**', r => {
    const large = r.request().url().endsWith('/large');
    r.fulfill({ body: large ? svg(1600, 1200) : svg(400, 300), contentType: 'image/svg+xml', headers: {'Access-Control-Allow-Origin':'*'} });
  });
  await p.goto('http://app.test/index.html');
  await p.evaluate(() => localStorage.clear());
  await p.waitForTimeout(300);
  await p.keyboard.press('Enter');
  await p.waitForTimeout(300);
  const vp = await p.$eval('#viewport', e => { const r = e.getBoundingClientRect(); return {x: r.left, y: r.top, w: r.width, h: r.height}; });
  const at = (x, y) => p.evaluate(`(${probe})(${x}, ${y})`);
  const near = (a, b) => Math.abs(a[0] - b[0]) < 0.003 && Math.abs(a[1] - b[1]) < 0.003;
  const tag = `[orientation ${orientation}]`;

  // Wheel at a point off-centre: that image point stays under the cursor, also after the
  // switch to the large preview.
  const mx = vp.w * 0.3, my = vp.h * 0.35;
  await p.mouse.move(vp.x + mx, vp.y + my);
  const p0 = await at(mx, my);
  for (let i = 0; i < 8; i++) { await p.mouse.wheel(0, -300); await p.waitForTimeout(40); }
  await p.waitForTimeout(400);
  const p1 = await at(mx, my);
  ok(near(p0, p1), `${tag} wheel zoom anchored at cursor ${p0} -> ${p1}`);
  ok((await p.$eval('#loupe-img', i => i.src)).endsWith('/large'), `${tag} switched to large preview`);

  // Keys: F fits, then Z (100 %) and + with the mouse at another point.
  await p.keyboard.press('f');
  const kx = vp.w * 0.7, ky = vp.h * 0.6;
  await p.mouse.move(vp.x + kx, vp.y + ky);
  const k0 = await at(kx, ky);
  await p.keyboard.press('z');
  await p.waitForTimeout(300);
  ok(near(k0, await at(kx, ky)), `${tag} Z zooms to 100 % at cursor`);
  await p.keyboard.press('+'); await p.keyboard.press('+'); await p.keyboard.press('-');
  ok(near(k0, await at(kx, ky)), `${tag} +/- anchored at cursor`);

  // Double-click toggles back to fit, and again to 100 % at the clicked point.
  await p.mouse.dblclick(vp.x + kx, vp.y + ky);
  await p.waitForTimeout(100);
  const d0 = await at(mx, my);
  await p.mouse.dblclick(vp.x + mx, vp.y + my);
  await p.waitForTimeout(100);
  ok(near(d0, await at(mx, my)), `${tag} double-click zooms at the clicked point`);

  // Mouse outside the image area: keys zoom around the centre.
  await p.keyboard.press('f');
  await p.mouse.move(vp.x + vp.w + 50, vp.y + 10);
  const c0 = await at(vp.w / 2, vp.h / 2);
  await p.keyboard.press('+');
  ok(near(c0, await at(vp.w / 2, vp.h / 2)), `${tag} mouse outside: keys zoom at centre`);

  // Anchoring off (toggle in the status bar, remembered): keys and wheel zoom at the centre.
  await p.keyboard.press('f');
  await p.click('#status-anchor');
  ok((await p.textContent('#status-anchor')).includes('mitten'), `${tag} toggle shows centre`);
  ok(await p.evaluate(() => localStorage.getItem('zoomAnchor')) === 'center', `${tag} choice remembered`);
  await p.mouse.move(vp.x + mx, vp.y + my);
  const e0 = await at(vp.w / 2, vp.h / 2);
  await p.keyboard.press('z');
  await p.mouse.wheel(0, -300);
  await p.waitForTimeout(100);
  ok(near(e0, await at(vp.w / 2, vp.h / 2)), `${tag} anchoring off: zoom at centre`);
  await p.click('#status-anchor');
  ok((await p.textContent('#status-anchor')).includes('pekaren'), `${tag} toggle back to cursor`);

  ok(errors.length === 0, `${tag} no page errors ${errors.join('|')}`);
  await b.close();
}

(async () => {
  const ok = (c, m) => { if (!c) { console.log('FAIL', m); process.exitCode = 1; } else console.log('ok  ', m); };
  for (const o of [1, 6, 8]) await run(o, ok);
})();
