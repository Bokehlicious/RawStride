// UI regression test: ingest dialog against a mocked Tauri backend.
// Needs Node + Playwright (not part of the app build):  node ui/tests/ingest-dialog.test.js
const { chromium } = require('playwright');
const fs = require('fs'); const path = require('path');
const WEB = path.join(__dirname, '..', 'web');
const mock = `
window.__calls = [];
window.__handlers = {};
const files = [{index:0,name:'A.ARW',size:1,ratable:true}];
const pv = {width:64,height:48,bytes:1,source:'t'};
window.__TAURI__ = { core: { invoke: async (cmd, a) => {
  window.__calls.push([cmd, JSON.parse(JSON.stringify(a || {}))]);
  switch (cmd) {
    case 'initial_folder': return null;
    case 'open_folder': return {generation:1, folder:a.path, files};
    case 'read_ratings': return [];
    case 'image_info': return {name:'A.ARW', orientation:1, previews:[pv], small:pv, medium:pv, large:pv};
    case 'pick_folder': return 'D:\\\\Valt';
    case 'ingest_volumes': return ['G:\\\\'];
    case 'ingest_preview': {
      const r = a.request;
      if (r.folder.includes('{bad}')) throw 'Mappmall: unknown variable {bad}';
      return {shots:2, files_to_copy:3, bytes_to_copy:150e6, present:1, renamed:1, no_exif:0, items_total:4,
        items:[{source:'TN1.ARW', rel:'2026/2026-10-26 '+r.job+'/TN1.ARW', action:'copy'},
               {source:'TN1.xmp', rel:'2026/2026-10-26 '+r.job+'/TN1.xmp', action:'copy'},
               {source:'TN2.ARW', rel:'2026/2026-10-26 '+r.job+'/TN2_1.ARW', action:'renamed'},
               {source:'TN3.ARW', rel:'x/TN3.ARW', action:'present'}]};
    }
    case 'ingest_start': setTimeout(() => {
        window.__handlers['ingest-progress']({payload:{phase:'copy',files_done:1,files_total:3,bytes_done:50e6,bytes_total:150e6,current:'TN1.ARW'}});
      }, 50);
      return null;
    case 'ingest_cancel': return null;
  }}}, event: { listen: async (name, cb) => { window.__handlers[name] = cb; } } };
window.PW_BASE = 'http://pw.test/';
`;
(async () => {
  const b = await chromium.launch(); const p = await b.newPage({viewport:{width:1200,height:850}});
  const errors = []; p.on('pageerror', e => errors.push(String(e)));
  await p.route('http://app.test/**', r => {
    const f = path.join(WEB, new URL(r.request().url()).pathname.slice(1) || 'index.html');
    let body = fs.readFileSync(f, 'utf8');
    if (f.endsWith('index.html')) body = body.replace('<script src="app.js">', `<script>${mock}</script><script src="app.js">`);
    r.fulfill({ body, contentType: f.endsWith('.js') ? 'text/javascript' : f.endsWith('.css') ? 'text/css' : 'text/html' });
  });
  await p.goto('http://app.test/index.html');
  await p.evaluate(() => localStorage.clear());
  await p.waitForTimeout(200);
  const ok = (c, m) => { if (!c) { console.log('FAIL', m); process.exitCode = 1; } else console.log('ok  ', m); };
  const lastPreview = () => p.evaluate(() => window.__calls.filter(c => c[0] === 'ingest_preview').slice(-1)[0]);

  await p.keyboard.press('Control+i');
  await p.waitForTimeout(200);
  ok(await p.isVisible('#ingest'), 'Ctrl+I opens the dialog');
  ok(!(await p.evaluate(() => document.getElementById('main').classList.contains('no-info'))), 'Ctrl+I does not toggle the info panel');
  ok((await p.inputValue('#ig-source')) === 'G:\\', 'detected card preselected');
  ok((await p.inputValue('#ig-folder')) === '{year}/{date} {job}' && (await p.inputValue('#ig-name')) === '{name}', 'default templates');
  ok((await p.textContent('#ig-error')).includes('Välj ett mål'), 'asks for destination');
  ok(await p.isDisabled('#ig-start'), 'start disabled without destination');

  await p.fill('#ig-dest', 'E:\\Foto');
  await p.fill('#ig-job', 'Bröllop');
  await p.waitForTimeout(450);
  const pr = await lastPreview();
  ok(pr[1].request.dest === 'E:\\Foto' && pr[1].request.job === 'Bröllop' && pr[1].request.backup === null, 'preview request ' + JSON.stringify(pr[1].request));
  const sum = await p.textContent('#ig-summary');
  ok(sum.includes('2 bilder') && sum.includes('150.0 MB') && sum.includes('1 finns redan') && sum.includes('namnkrock'), 'summary: ' + sum);
  ok((await p.textContent('#ig-list')).includes('2026-10-26 Bröllop/TN1.ARW'), 'plan list');
  ok(!(await p.isDisabled('#ig-start')), 'start enabled');

  // Typing digits in the dialog must not rate the image behind it.
  await p.fill('#ig-seq', '');
  await p.type('#ig-seq', '5');
  ok(!(await p.evaluate(() => window.__calls.some(c => c[0] === 'set_rating'))), 'keys stay in the dialog');

  // Bad template
  await p.fill('#ig-folder', '{bad}');
  await p.waitForTimeout(450);
  ok(await p.evaluate(() => document.getElementById('ig-folder').classList.contains('bad')), 'bad template marked');
  ok(await p.isDisabled('#ig-start'), 'start disabled on error');
  await p.fill('#ig-folder', '{year}/{date} {job}');
  await p.waitForTimeout(450);

  // Backup + IPTC + Enter does not close
  await p.check('#ig-backup-on');
  await p.fill('#ig-backup', 'F:\\Backup');
  await p.click('#ig-iptc-box summary');
  await p.fill('#ig-creator', 'Tobias');
  await p.fill('#ig-copyright', '© 2026 Tobias');
  await p.press('#ig-copyright', 'Enter');
  ok(await p.isVisible('#ingest'), 'Enter does not close');
  ok((await p.textContent('#ig-iptc-state')).includes('2 fält'), 'IPTC state');
  await p.check('#ig-eject');
  await p.waitForTimeout(450);

  await p.click('#ig-start');
  await p.waitForTimeout(200);
  const st = await p.evaluate(() => window.__calls.filter(c => c[0] === 'ingest_start')[0][1].request);
  ok(st.backup === 'F:\\Backup' && st.iptc.creator === 'Tobias' && st.eject === true && st.first_seq === 5, 'start request ' + JSON.stringify(st));
  ok(await p.isVisible('#ig-progress') && (await p.textContent('#ig-progress-text')).includes('1 / 3'), 'progress shown');
  ok(await p.isDisabled('#ig-dest'), 'fields locked while running');
  await p.click('#ig-cancel');
  ok(await p.evaluate(() => window.__calls.some(c => c[0] === 'ingest_cancel')), 'cancel sent');

  await p.evaluate(() => window.__handlers['ingest-done']({payload:{copied:3,present:1,bytes:150e6,seconds:1.5,cancelled:false,
    failed:[], iptc_errors:[], eject:'G: är utmatat och kan tas ut.', folders:[['E:\\Foto\\2026\\2026-10-26 Bröllop',3],['E:\\Foto\\x',1]], error:null}}));
  const res = await p.textContent('#ig-result');
  ok(res.includes('3 filer kopierade') && res.includes('100 MB/s') && res.includes('utmatat'), 'result: ' + res);
  ok(!(await p.isDisabled('#ig-dest')), 'fields unlocked');
  await p.click('#ig-open');
  await p.waitForTimeout(200);
  ok(!(await p.isVisible('#ingest')), 'dialog closed');
  const of = await p.evaluate(() => window.__calls.filter(c => c[0] === 'open_folder').slice(-1)[0][1].path);
  ok(of === 'E:\\Foto\\2026\\2026-10-26 Bröllop', 'opens largest folder: ' + of);

  // Settings remembered (except job), job kept within the session
  const saved = await p.evaluate(() => JSON.parse(localStorage.getItem('ingest')));
  ok(saved.dest === 'E:\\Foto' && saved.job === '' && saved.iptc.creator === 'Tobias', 'settings saved');
  await p.click('#ingest-btn');
  await p.waitForTimeout(200);
  ok((await p.inputValue('#ig-job')) === 'Bröllop' && (await p.inputValue('#ig-dest')) === 'E:\\Foto', 'reopen keeps values');
  await p.screenshot({ path: process.env.SHOT || '/dev/null' }).catch(() => {});
  await p.keyboard.press('Escape');
  ok(!(await p.isVisible('#ingest')), 'Escape closes');
  ok(errors.length === 0, 'no page errors ' + errors.join('|'));
  await b.close();
})();
