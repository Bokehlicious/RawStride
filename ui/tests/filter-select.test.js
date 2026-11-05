// UI regression test: rating/label filter and multi-selection, against a mocked Tauri backend.
// Needs Node + Playwright (not part of the app build):  node ui/tests/filter-select.test.js
const { chromium } = require('playwright');
const fs = require('fs'); const path = require('path');
const WEB = path.join(__dirname, '..', 'web');
const img = Buffer.from('/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAYEBQYFBAYGBQYHBwYIChAKCgkJChQODwwQFxQYGBcUFhYaHSUfGhsjHBYWICwgIyYnKSopGR8tMC0oMCUoKSj/2wBDAQcHBwoIChMKChMoGhYaKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCj/wAARCAAwAEADASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwDPooor3TygooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKACiiigAooooAKKKKAP/2Q==', 'base64'); // 64×48 JPEG
const mock = `
window.__calls = [];
const N = 12;
const files = Array.from({length:N},(_,i)=>({index:i,name:'IMG_'+String(i).padStart(3,'0')+(i==11?'.png':'.ARW'),size:1000,ratable:i!==11}));
const ratings = {1:{stars:3,label:null},2:{stars:5,label:'Red'},3:{stars:-1,label:null},4:{stars:null,label:'Red'}};
const pv = {width:64,height:48,bytes:1,source:'t'};
window.__TAURI__ = { core: { invoke: async (cmd, a) => {
  window.__calls.push([cmd, a]);
  switch (cmd) {
    case 'initial_folder': return 'D:/x';
    case 'open_folder': return {generation:1, folder:'D:/x', files};
    case 'read_ratings': return Object.entries(ratings).map(([i,r])=>({index:+i,...r}));
    case 'image_info': return {name:files[a.index].name, orientation:1, previews:[pv], small:pv, medium:pv, large:pv, width:64, height:48};
    case 'set_rating': return {index:a.index, stars:a.stars, label:a.label};
  }}}, event: { listen: () => {} } };
window.PW_BASE = 'http://pw.test/';
`;
(async () => {
  const b = await chromium.launch(); const p = await b.newPage({viewport:{width:1200,height:800}});
  const errors = []; p.on('pageerror', e => errors.push(String(e)));
  await p.route('http://app.test/**', r => {
    const f = path.join(WEB, new URL(r.request().url()).pathname.slice(1) || 'index.html');
    let body = fs.readFileSync(f, 'utf8');
    if (f.endsWith('index.html')) body = body.replace('<script src="app.js">', `<script>${mock}</script><script src="app.js">`);
    r.fulfill({ body, contentType: f.endsWith('.js') ? 'text/javascript' : f.endsWith('.css') ? 'text/css' : 'text/html' });
  });
  await p.route('http://pw.test/**', r => r.fulfill({ body: img, contentType: 'image/jpeg', headers: {'Access-Control-Allow-Origin':'*'} }));
  await p.goto('http://app.test/index.html');
  await p.waitForTimeout(400);
  const vis = () => p.$$eval('.cell:not([hidden])', cs => cs.map(c => +c.dataset.index));
  const status = () => p.textContent('#status-pos');
  const ok = (c, m) => { if (!c) { console.log('FAIL', m); process.exitCode = 1; } else console.log('ok  ', m); };

  ok((await vis()).length === 12, 'all visible');
  await p.selectOption('#filter-stars', '3');
  ok(JSON.stringify(await vis()) === '[1,2]', '>=3 stars -> 1,2: ' + JSON.stringify(await vis()));
  ok((await status()).startsWith('1 / 2 (av 12)'), 'status ' + await status());
  await p.selectOption('#filter-label', 'Red');
  ok(JSON.stringify(await vis()) === '[2]', '>=3 & Red -> 2');
  await p.selectOption('#filter-stars', 'all');
  ok(JSON.stringify(await vis()) === '[2,4]', 'Red -> 2,4');
  await p.selectOption('#filter-label', 'Purple');
  ok((await vis()).length === 0 && (await status()).includes('Inga bilder'), 'empty filter');
  await p.selectOption('#filter-label', 'all');
  await p.selectOption('#filter-stars', 'rejected');
  ok(JSON.stringify(await vis()) === '[3]', 'rejected -> 3');
  await p.selectOption('#filter-stars', 'unrated');
  ok(JSON.stringify(await vis()) === '[0,4,5,6,7,8,9,10,11]', 'unrated');

  // Rate current (0) under "unrated": stays until moving on
  await p.focus('#grid');
  await p.keyboard.press('Digit2');
  await p.waitForTimeout(100);
  ok((await vis()).includes(4), 'rated image stays visible');
  await p.keyboard.press('ArrowRight');
  ok(!(await vis()).includes(4), 'disappears after moving on');
  ok((await status()).includes('IMG_005'), 'current is 4: ' + await status());

  await p.selectOption('#filter-stars', 'all');
  await p.focus('#grid');
  // Multi-select: click 5, shift+click 8, press 7 (Yellow)
  await p.click('.cell[data-index="5"]');
  await p.click('.cell[data-index="8"]', { modifiers: ['Shift'] });
  ok((await p.textContent('#status-sel')) === '4 markerade', 'range 4 marked');
  await p.click('.cell[data-index="10"]', { modifiers: ['Control'] });
  ok((await p.textContent('#status-sel')) === '5 markerade', 'ctrl adds');
  await p.click('.cell[data-index="11"]', { modifiers: ['Control'] }); // non-ratable
    await p.keyboard.press('Digit7');
  await p.waitForTimeout(200);
  const sets = await p.evaluate(() => window.__calls.filter(c => c[0]==='set_rating').slice(-5).map(c => [c[1].index, c[1].label]));
  ok(JSON.stringify(sets) === '[[5,"Yellow"],[6,"Yellow"],[7,"Yellow"],[8,"Yellow"],[10,"Yellow"]]', 'label to all ratable marked: ' + JSON.stringify(sets));
  ok(!(await p.isVisible('#toast')), 'no toast for skipped non-ratable in batch');
  await p.keyboard.press('Digit7'); // toggle off for all
  await p.waitForTimeout(200);
  const offs = await p.evaluate(() => window.__calls.filter(c => c[0]==='set_rating').slice(-5).map(c => c[1].label));
  ok(offs.every(l => l === null), 'toggle off for all');
  await p.keyboard.press('Escape');
  ok((await p.textContent('#status-sel')) === '', 'escape clears');
  // Shift+arrow
  await p.keyboard.press('Shift+ArrowLeft'); await p.keyboard.press('Shift+ArrowLeft');
  ok((await p.textContent('#status-sel')) === '3 markerade', 'shift+arrow extends');
  await p.keyboard.press('Control+a');
  ok((await p.textContent('#status-sel')) === '12 markerade', 'ctrl+a');
  await p.keyboard.press('ArrowLeft');
  ok((await p.textContent('#status-sel')) === '', 'plain arrow clears');
  // Loupe with filter: navigation stays within filtered set
  await p.selectOption('#filter-label', 'Red');
  await p.focus('#grid');
  await p.keyboard.press('Home');
  await p.keyboard.press('Enter');
  await p.waitForTimeout(150);
  const s1 = await status(); await p.keyboard.press('ArrowRight'); await p.waitForTimeout(100); const s2 = await status();
  ok(s1.includes('IMG_002') && s2.includes('IMG_004'), 'loupe within filter: ' + s1 + ' -> ' + s2);
  ok(errors.length === 0, 'no page errors ' + errors.join('|'));
  await b.close();
})();
