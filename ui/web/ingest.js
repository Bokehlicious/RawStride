// PhotoWorkflow – ingest dialog (ADR 0005). Uses invoke/listen/toast/openFolder from app.js.
"use strict";

const ig = {
  dlg: $("ingest"), source: $("ig-source"), pickSource: $("ig-source-pick"), rescan: $("ig-rescan"),
  dest: $("ig-dest"), backupOn: $("ig-backup-on"), backup: $("ig-backup"),
  folder: $("ig-folder"), name: $("ig-name"), job: $("ig-job"), seq: $("ig-seq"),
  iptcBox: $("ig-iptc-box"), iptcState: $("ig-iptc-state"), eject: $("ig-eject"),
  summary: $("ig-summary"), error: $("ig-error"), list: $("ig-list"),
  progress: $("ig-progress"), bar: $("ig-bar"), progressText: $("ig-progress-text"),
  result: $("ig-result"), close: $("ig-close"), cancel: $("ig-cancel"), open: $("ig-open"),
  start: $("ig-start"),
};
const IPTC_FIELDS = ["creator", "copyright", "credit", "email", "phone", "url"];
const IG_DEFAULTS = {
  source: "", dest: "", backupOn: false, backup: "",
  folder: "{year}/{date} {job}", name: "{name}", job: "", seq: 1, eject: false,
  iptc: {},
};
const igState = { running: false, preview: null, doneFolders: [], timer: 0, seqNo: 0, job: null };

function igSettings() {
  let s = {};
  try { s = JSON.parse(localStorage.getItem("ingest") || "{}") || {}; } catch (_) {}
  return { ...IG_DEFAULTS, ...s, iptc: { ...(s.iptc || {}) } };
}

function igSave() {
  const s = {
    source: ig.source.value, dest: ig.dest.value.trim(), backupOn: ig.backupOn.checked,
    backup: ig.backup.value.trim(), folder: ig.folder.value, name: ig.name.value,
    job: ig.job.value, seq: Number(ig.seq.value) || 1, eject: ig.eject.checked, iptc: {},
  };
  for (const f of IPTC_FIELDS) s.iptc[f] = $(`ig-${f}`).value;
  // The job name belongs to one shoot; keep it only for this session.
  igState.job = s.job;
  try { localStorage.setItem("ingest", JSON.stringify({ ...s, job: "" })); } catch (_) {}
  return s;
}

function igRequest() {
  const s = igSave();
  return {
    source: s.source, dest: s.dest, backup: s.backupOn ? s.backup : null,
    folder: s.folder, name: s.name, job: s.job, first_seq: Math.max(0, Math.floor(s.seq)),
    iptc: s.iptc, eject: s.eject,
  };
}

const fmtBytes = (b) => b >= 1e9 ? `${(b / 1e9).toFixed(2)} GB` : `${(b / 1e6).toFixed(1)} MB`;

async function igFillSources(keep) {
  let vols = [];
  try { vols = await invoke("ingest_volumes"); } catch (_) {}
  const opts = vols.map((v) => [v, `${v}  (kort)`]);
  if (keep && !vols.includes(keep)) opts.push([keep, keep]);
  if (!opts.length) opts.push(["", "Inget kort hittat – välj en mapp"]);
  ig.source.replaceChildren(...opts.map(([v, t]) => {
    const o = document.createElement("option");
    o.value = v; o.textContent = t;
    return o;
  }));
  // A detected card wins over the last folder.
  ig.source.value = vols[0] || keep || "";
}

async function openIngest() {
  if (ig.dlg.open) return;
  if (igState.running) { ig.dlg.showModal(); return; }
  const s = igSettings();
  ig.dest.value = s.dest; ig.backupOn.checked = s.backupOn; ig.backup.value = s.backup;
  ig.folder.value = s.folder; ig.name.value = s.name; ig.job.value = igState.job ?? s.job;
  ig.seq.value = s.seq; ig.eject.checked = s.eject;
  for (const f of IPTC_FIELDS) $(`ig-${f}`).value = s.iptc[f] || "";
  igSyncBackup();
  igIptcState();
  igShowRunning(false);
  ig.result.hidden = true;
  ig.open.hidden = true;
  ig.dlg.showModal();
  await igFillSources(s.source);
  igPreview(true);
}

function igSyncBackup() {
  ig.backup.disabled = !ig.backupOn.checked;
  ig.dlg.querySelector('[data-pick="ig-backup"]').disabled = !ig.backupOn.checked;
}

function igIptcState() {
  const n = IPTC_FIELDS.filter((f) => $(`ig-${f}`).value.trim()).length;
  ig.iptcState.textContent = n ? `(${n} fält ifyllda)` : "(av)";
}

function igSchedule(rescan = false) {
  clearTimeout(igState.timer);
  igState.timer = setTimeout(() => igPreview(rescan), rescan ? 0 : 300);
}

async function igPreview(rescan = false) {
  if (igState.running) return;
  const req = igRequest();
  const my = ++igState.seqNo;
  ig.start.disabled = true;
  ig.error.hidden = true;
  ig.folder.classList.remove("bad");
  ig.name.classList.remove("bad");
  if (!req.source) { igShowError("Välj en källa."); return; }
  if (!req.dest) { igShowError("Välj ett mål."); return; }
  ig.summary.textContent = rescan ? "Läser källan …" : "Beräknar …";
  try {
    const p = await invoke("ingest_preview", { request: req, rescan });
    if (my !== igState.seqNo) return;
    igState.preview = p;
    igRenderPreview(p);
  } catch (e) {
    if (my !== igState.seqNo) return;
    const msg = String(e);
    if (msg.startsWith("Mappmall")) ig.folder.classList.add("bad");
    if (msg.startsWith("Filnamnsmall")) ig.name.classList.add("bad");
    igShowError(msg);
  }
}

function igShowError(msg) {
  igState.preview = null;
  ig.summary.textContent = "";
  ig.list.replaceChildren();
  ig.error.textContent = msg;
  ig.error.hidden = false;
  ig.start.disabled = true;
}

function igRenderPreview(p) {
  const parts = [];
  if (!p.shots) {
    parts.push("Inga bilder hittades i källan.");
  } else {
    parts.push(`${p.shots} bilder · ${p.files_to_copy} filer att kopiera (${fmtBytes(p.bytes_to_copy)})`);
    if (p.present) parts.push(`${p.present} finns redan`);
    if (p.renamed) parts.push(`${p.renamed} får nytt namn pga namnkrock`);
    if (p.no_exif) parts.push(`${p.no_exif} utan EXIF-datum (fildatum används)`);
  }
  ig.summary.textContent = parts.join(" · ");
  const label = { copy: "", renamed: "  (namnkrock)", present: "  (finns redan)", backup: "  (bara backup)" };
  const rows = p.items.map((i) => {
    const d = document.createElement("div");
    d.className = `a-${i.action}`;
    d.textContent = `${i.source}  →  ${i.rel}${label[i.action]}`;
    return d;
  });
  if (p.items_total > p.items.length) {
    const d = document.createElement("div");
    d.className = "a-present";
    d.textContent = `… och ${p.items_total - p.items.length} till`;
    rows.push(d);
  }
  ig.list.replaceChildren(...rows);
  ig.start.disabled = p.files_to_copy === 0;
}

function igShowRunning(on) {
  igState.running = on;
  ig.progress.hidden = !on;
  ig.cancel.hidden = !on;
  ig.start.hidden = on;
  ig.close.textContent = on ? "Dölj" : "Stäng";
  for (const e of ig.dlg.querySelectorAll("input, select, #ig-source-pick, #ig-rescan, [data-pick]")) {
    e.disabled = on || (e.id === "ig-backup" || e.dataset.pick === "ig-backup" ? !ig.backupOn.checked : false);
  }
}

async function igStart() {
  const req = igRequest();
  ig.result.hidden = true;
  ig.open.hidden = true;
  ig.bar.value = 0;
  ig.progressText.textContent = "Startar …";
  igShowRunning(true);
  try {
    await invoke("ingest_start", { request: req });
  } catch (e) {
    igShowRunning(false);
    igShowError(String(e));
  }
}

function igOnProgress(p) {
  if (p.phase === "copy") {
    ig.bar.max = Math.max(1, p.bytes_total);
    ig.bar.value = p.bytes_done;
    ig.progressText.textContent =
      `Kopierar och verifierar ${p.files_done} / ${p.files_total} · ` +
      `${fmtBytes(p.bytes_done)} av ${fmtBytes(p.bytes_total)} · ${p.current}`;
  } else if (p.phase === "iptc") {
    ig.bar.max = Math.max(1, p.files_total);
    ig.bar.value = p.files_done;
    ig.progressText.textContent = `Skriver IPTC ${p.files_done + 1} / ${p.files_total} · ${p.current}`;
  } else if (p.phase === "eject") {
    ig.progressText.textContent = `Matar ut ${p.current} …`;
  }
}

function igOnDone(d) {
  igShowRunning(false);
  ig.start.hidden = false;
  const lines = [];
  if (d.error) {
    lines.push(`<div class="bad">${escapeHtml(d.error)}</div>`);
  } else {
    const speed = d.seconds > 0 ? ` · ${(d.bytes / 1e6 / d.seconds).toFixed(0)} MB/s` : "";
    lines.push(`<div>${d.cancelled ? "Avbruten" : "Klar"}: ${d.copied} filer kopierade och verifierade ` +
      `(${fmtBytes(d.bytes)} på ${d.seconds.toFixed(1)} s${speed})` +
      `${d.present ? `, ${d.present} fanns redan` : ""}.</div>`);
    for (const f of d.failed.slice(0, 20)) lines.push(`<div class="bad">Misslyckades: ${escapeHtml(f)}</div>`);
    if (d.failed.length > 20) lines.push(`<div class="bad">… och ${d.failed.length - 20} till</div>`);
    for (const f of d.iptc_errors.slice(0, 10)) lines.push(`<div class="bad">IPTC: ${escapeHtml(f)}</div>`);
    if (d.eject) lines.push(`<div>${escapeHtml(d.eject)}</div>`);
    if (d.folders.length > 1) {
      lines.push(`<div class="muted">Mappar: ${d.folders.map(([f, n]) => `${escapeHtml(f)} (${n})`).join(", ")}</div>`);
    }
  }
  ig.result.innerHTML = lines.join("");
  ig.result.hidden = false;
  igState.doneFolders = d.folders || [];
  ig.open.hidden = !igState.doneFolders.length;
  ig.open.textContent = igState.doneFolders.length > 1
    ? `Öppna ${shortName(igState.doneFolders[0][0])} i visaren` : "Öppna i visaren";
  if (!ig.dlg.open) toast(d.error || `Importen är ${d.cancelled ? "avbruten" : "klar"}: ${d.copied} filer.`);
  // Refresh the plan (now "already present"), unless the card was ejected.
  if (!d.eject) igPreview(false);
}

function shortName(p) {
  const parts = p.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] || p;
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

// ---------- wiring ----------

$("ingest-btn").addEventListener("click", openIngest);
document.addEventListener("keydown", (e) => {
  if (e.ctrlKey && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "i") {
    e.preventDefault();
    e.stopImmediatePropagation();
    openIngest();
  }
}, true);
// Keys typed in the dialog must not reach the viewer (ratings, navigation).
ig.dlg.addEventListener("keydown", (e) => e.stopPropagation());

for (const el2 of [ig.dest, ig.backup, ig.folder, ig.name, ig.job, ig.seq]) {
  el2.addEventListener("input", () => igSchedule());
}
for (const f of IPTC_FIELDS) $(`ig-${f}`).addEventListener("input", () => { igIptcState(); igSave(); });
ig.eject.addEventListener("change", igSave);
ig.backupOn.addEventListener("change", () => { igSyncBackup(); igSchedule(); });
ig.source.addEventListener("change", () => igSchedule(true));
ig.rescan.addEventListener("click", async () => {
  await igFillSources(ig.source.value);
  igSchedule(true);
});
ig.pickSource.addEventListener("click", async () => {
  const p = await invoke("pick_folder");
  if (!p) return;
  await igFillSources(p);
  ig.source.value = p;
  igSchedule(true);
});
for (const b of ig.dlg.querySelectorAll("[data-pick]")) {
  b.addEventListener("click", async () => {
    const p = await invoke("pick_folder");
    if (!p) return;
    $(b.dataset.pick).value = p;
    igSchedule();
  });
}
ig.start.addEventListener("click", igStart);
// Enter in a field must not submit (and close) the dialog.
$("ingest-form").addEventListener("submit", (e) => e.preventDefault());
ig.cancel.addEventListener("click", () => {
  ig.progressText.textContent = "Avbryter …";
  invoke("ingest_cancel");
});
ig.close.addEventListener("click", () => ig.dlg.close());
ig.dlg.addEventListener("cancel", (e) => { if (igState.running) { e.preventDefault(); ig.dlg.close(); } });
ig.open.addEventListener("click", () => {
  const f = igState.doneFolders[0];
  if (!f) return;
  ig.dlg.close();
  openFolder(f[0]);
});
listen("ingest-progress", (ev) => igOnProgress(ev.payload));
listen("ingest-done", (ev) => igOnDone(ev.payload));
