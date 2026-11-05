// UI regression test: error handling of RAW rendering (M2) against a mocked backend.
// Damaged RAW keeps the preview with a warning chip; an unreadable file empties the loupe; late
// results of earlier images never change the image on screen.
// Needs Node + Playwright (not part of the app build):  node ui/tests/raw-errors.test.js
const { chromium } = require('playwright');
const fs = require('fs'); const path = require('path');
const WEB = path.join(__dirname, '..', 'web');
const svg = (w, h, c) => `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">` +
  `<rect width="${w}" height="${h}" fill="${c}"/></svg>`;
const mock = `
window.__calls = []; window.__h = {};
const files = [{index:0,name:'TN100423.ARW'},{index:1,name:'TN100426_damaged.ARW'},{index:2,name:'garbage.ARW'},
  {index:3,name:'jpeg_as_raw.ARW'},{index:4,name:'P1240231.RW2'},{index:5,name:'dng_cut50.dng'}].map(f => ({...f, size:1, ratable:true}));
const med = {width:400,height:300,bytes:1,source:'m'};
window.__TAURI__ = { core: { invoke: async (cmd, a) => {
  window.__calls.push([cmd, JSON.parse(JSON.stringify(a || {}))]);
  switch (cmd) {
    case 'initial_folder': return 'D:/x';
    case 'open_folder': return {generation:1, folder:'D:/x', files};
    case 'read_ratings': return [];
    case 'image_info':
      if (a.index === 2) throw 'format error: not a TIFF/RAW file';
      if (a.index === 5) return {name:files[5].name, orientation:1, previews:[], small:null, medium:null, large:null, width:3200, height:2400};
      return {name:files[a.index].name, orientation:1, previews:[med], small:med, medium:med, large:med,
        width:3200, height:2400};
    case 'raw_view': return {enabled:true, want_half: a.index !== 4 && a.index != null, can_tile: a.index !== 2 && a.index != null, grid:null};
    default: return null;
  }}}, event: { listen: async (n, cb) => { window.__h[n] = cb; } } };
window.PW_BASE = 'http://pw.test/';
`;
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
    const body = u.endsWith('/rawhalf') ? svg(800, 600, '#486') : /tile\//.test(u) ? svg(1024, 1024, '#a33') : svg(400, 300, '#468');
    r.fulfill({ body, contentType: 'image/svg+xml', headers: {'Access-Control-Allow-Origin':'*'} });
  });
  await p.goto('http://app.test/index.html');
  await p.evaluate(() => localStorage.clear());
  await p.waitForTimeout(300);
  const calls = (c) => p.evaluate((c) => window.__calls.filter(x => x[0] === c).map(x => x[1]), c);
  const clear = () => p.evaluate(() => { window.__calls.length = 0; });
  const emit = (n, payload) => p.evaluate(([n, payload]) => window.__h[n]({ payload }), [n, payload]);
  const src = () => p.$eval('#loupe-img', i => i.getAttribute('src'));
  const warn = () => p.$eval('#loupe-rawwarn', e => e.hidden ? null : e.textContent);
  const badge = () => p.textContent('#loupe-badge');
  const tiles = () => p.$$eval('#loupe-tiles img', a => a.length);
  const go = async (k) => { await p.keyboard.press(k); await p.waitForTimeout(250); };
  const vp = await p.$eval('#viewport', e => { const r = e.getBoundingClientRect(); return {x: r.left, y: r.top}; });

  await p.keyboard.press('Enter');
  await p.waitForTimeout(300);
  ok(await warn() === null, 'good image: no warning chip');
  const seq0 = (await calls('raw_view')).map(c => c.seq);
  ok(seq0.length && seq0.every(n => n > 0), 'raw_view calls carry a sequence number');

  // A good image with tiles on screen, then the damaged one.
  await emit('raw-ready', { generation: 1, index: 0, kind: 'half', full_width: 3200, full_height: 2400 });
  await p.waitForTimeout(300);
  ok((await src()).endsWith('/0/rawhalf'), 'good A7R: half-size RAW shown');
  await go('ArrowRight');
  ok((await src()).endsWith('/1/medium'), 'damaged A7R: embedded preview first');
  await clear();
  await emit('raw-failed', { generation: 1, index: 1, kind: 'failed',
    message: 'LibRaw: D:/x/TN100426_damaged.ARW: damaged: RAW data incomplete, 37000000 bytes missing' });
  await p.waitForTimeout(200);
  ok((await src()).endsWith('/1/medium'), 'damaged A7R: preview stays');
  ok(/Skadad RAW/.test(await warn() || ''), 'damaged A7R: warning chip "' + await warn() + '"');
  ok(/skadad eller ofullständig/.test(await badge()), 'damaged A7R: badge explains');
  ok(!(await badge()).includes('zooma in för RAW-rutor'), 'damaged A7R: no promise of RAW tiles');
  // A late raw-ready for the damaged file (should never come) must not replace the preview.
  await emit('raw-ready', { generation: 1, index: 1, kind: 'half', full_width: 3200, full_height: 2400 });
  await p.waitForTimeout(300);
  ok((await src()).endsWith('/1/medium'), 'damaged A7R: no RAW image after a failure');
  // Zooming: M1 behaviour, no unpack, no tiles.
  await p.mouse.move(vp.x + 400, vp.y + 300);
  for (let i = 0; i < 5; i++) { await p.mouse.wheel(0, -300); await p.waitForTimeout(30); }
  await p.evaluate(() => { fitToWindow(); zoomTo100(); });
  await p.waitForTimeout(200);
  ok((await calls('raw_prepare')).length === 0 && (await calls('raw_tiles')).length === 0, 'damaged A7R: no unpack, no tiles');
  ok(Math.abs(await p.evaluate(() => view.scale) - 1) < 1e-6, 'damaged A7R: 100 % = the embedded preview (M1)');
  await p.evaluate(() => fitToWindow());

  // Unreadable file: the loupe is emptied, nothing of the previous image stays.
  await go('ArrowRight');
  await p.waitForTimeout(200);
  ok(await src() === null, 'garbage: no image (previous image not kept)');
  ok(await tiles() === 0, 'garbage: no tiles');
  ok(/Filen kan inte läsas/.test(await badge()), 'garbage: badge "' + await badge() + '"');
  ok(await warn() === null, 'garbage: no fallback chip (there is no fallback image)');
  ok((await p.textContent('#status-pos')).includes('garbage.ARW'), 'garbage: status names the file');
  const last = (await calls('raw_view')).pop();
  ok(last && last.index === 2, 'garbage: backend told (stops the previous image)');

  // JPEG named .ARW: LibRaw rejects it, the image itself is shown, chip says "kunde inte avkodas".
  await go('ArrowRight');
  await emit('raw-failed', { generation: 1, index: 3, kind: 'failed', message: 'LibRaw: open: Unsupported file format or not RAW file' });
  await p.waitForTimeout(200);
  ok((await src()).endsWith('/3/medium'), 'jpeg_as_raw: image shown');
  ok(/kunde inte avkodas/.test(await warn() || ''), 'jpeg_as_raw: chip "' + await warn() + '"');

  // Late results of earlier images while another image is on screen change nothing.
  await go('ArrowRight'); // RW2
  const rw2 = await src();
  await emit('raw-ready', { generation: 1, index: 0, kind: 'half', full_width: 3200, full_height: 2400 });
  await emit('raw-tile', { generation: 1, index: 0, col: 0, row: 0 });
  await emit('raw-failed', { generation: 1, index: 0, kind: 'failed', message: 'damaged: late' });
  await emit('raw-grid', { generation: 1, index: 0, width: 3200, height: 2400, tile: 1024 });
  await p.waitForTimeout(400);
  ok(await src() === rw2 && await tiles() === 0 && await warn() === null, 'late results of image 0 do not touch the RW2 on screen');
  // ... and they are kept for image 0: back there, its failure shows.
  await go('Home');
  await p.waitForTimeout(200);
  ok(/Skadad RAW/.test(await warn() || ''), 'image 0 shows its own (late) failure when visited');

  // Rapid switching across good, damaged, unreadable files with events interleaved.
  for (let i = 0; i < 40; i++) {
    await p.keyboard.press(i % 7 < 4 ? 'ArrowRight' : 'ArrowLeft');
    if (i % 3 === 0) await emit('raw-tile', { generation: 1, index: i % 5, col: 1, row: 1 });
    if (i % 5 === 0) await emit('raw-ready', { generation: 1, index: (i + 1) % 5, kind: 'half', full_width: 3200, full_height: 2400 });
    await p.waitForTimeout(15 + (i % 4) * 20);
  }
  await go('End');
  await go('ArrowLeft'); // ends on the RW2
  await p.waitForTimeout(400);
  const st = await p.evaluate(() => ({ idx: view.index, cur: state.current, src: document.getElementById('loupe-img').getAttribute('src') }));
  ok(st.idx === 4 && st.cur === 4 && st.src.endsWith('/4/medium'), 'after rapid switching: the RW2 on screen, its own preview ' + JSON.stringify(st));
  ok(await tiles() === 0 && await warn() === null, 'after rapid switching: no foreign tiles or warnings');
  const seqs = (await calls('raw_view')).map(c => c.seq);
  ok(seqs.every((n, i) => i === 0 || n > seqs[i - 1]), 'raw_view sequence numbers increase');
  await go('Home');
  await p.waitForTimeout(300);
  await go('ArrowRight');
  await go('ArrowRight'); // garbage again
  ok(await src() === null, 'garbage after rapid switching: still empty');

  // A damaged file whose embedded preview was cut off too (DNG with the preview at the end):
  // nothing of the previous image stays, and the chip does not promise a preview.
  await go('End'); // RW2 with tiles possible
  await go('ArrowRight'); // index 5
  await emit('raw-failed', { generation: 1, index: 5, kind: 'failed', message: 'LibRaw: x.dng: damaged: RAW data incomplete, 41662 bytes missing' });
  await p.waitForTimeout(200);
  ok(await src() === null && await tiles() === 0, 'no-preview damaged file: empty loupe, no foreign tiles');
  ok(/ingen inbäddad bild att visa/.test(await warn() || ''), 'no-preview damaged file: chip "' + await warn() + '"');
  ok(/skadad eller ofullständig/.test(await badge()), 'no-preview damaged file: badge explains');
  ok(errors.length === 0, 'no page errors ' + errors.join(' | '));
  await b.close();
})();
