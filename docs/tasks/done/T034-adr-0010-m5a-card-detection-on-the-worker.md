# T34. ADR 0010 M5a: card detection on the worker, dialog keys and texts (plain Rust) — Done

- **Crate:** `pw-loupe`. **Files:** `crates/pw-loupe/Cargo.toml` (add `pw-core.workspace = true`;
  internal), `crates/pw-loupe/src/import_worker.rs`, `crates/pw-loupe/src/import_dialog.rs`, a new
  `crates/pw-loupe/src/import_text.rs`, `crates/pw-loupe/src/lib.rs`. **Not** `main.rs`.
- **Why:** M5 draws the import dialog (T35, T36). Everything that can be tested without a window is
  done first, here.
- **What:**
  1. **Card detection on the worker** (it checks drives A–Z, which is file I/O): `ImportWorker::cards()
     -> Result<RequestId, SubmitError>`; the worker answers `ImportEvent::Cards { id, cards: Vec<PathBuf> }`
     from `pw_ingest::card_volumes()`.
  2. **Dialog additions** in `ImportDialog`: `can_start(&self) -> bool` (true exactly when `start()`
     would return a request, without changing anything) and `rescan(&mut self, now: Instant)` (marks the
     form as changed with a new scan, for a "Sök igen" button; ignored while running).
  3. **Keys** (plain functions):
     - `opens_import(key, ctrl, shift, alt) -> bool`: true only for **Ctrl+I** (no Shift, no Alt). The
       loupe's `I` (info) without Ctrl is unchanged.
     - `dialog_key_action(key, ctrl, alt) -> Option<DialogKey>` with `DialogKey::{Close, Start}`:
       Escape → `Close`, Enter → `Start`; nothing with Ctrl or Alt.
  4. **Swedish texts**, the same as the Tauri dialog (`ui/web/ingest.js`, `igRenderPreview`,
     `igOnProgress`, `igOnDone`), with `pw_core::format_bytes` for sizes:
     - `preview_summary(&Preview) -> String`: "Inga bilder hittades i källan." when there are no shots;
       otherwise "{shots} bilder · {files} filer att kopiera ({bytes})", followed, when not zero, by
       "{n} finns redan", "{n} får nytt namn pga namnkrock" and "{n} utan EXIF-datum (fildatum används)",
       all joined with " · ".
     - `preview_line(&PreviewLine) -> String`: "{source}  →  {rel}" plus "  (namnkrock)",
       "  (finns redan)" or "  (bara backup)" for those actions; `more_lines(&Preview) -> Option<String>`:
       "… och {n} till" when `items_total` is larger than the lines shown.
     - `progress_text(&pw_ingest::Progress) -> String`: "Kopierar och verifierar {done} / {total} ·
       {bytes done} av {bytes total} · {file name of current}".
     - `done_lines(&ImportDone) -> Vec<DoneLine>` with `DoneLine { text: String, error: bool }`: with an
       `error`, only that line (as an error). Otherwise "Klar: " or "Avbruten: " + "{copied} filer kopierade
       och verifierade ({bytes} på {seconds:.1} s · {MB/s} MB/s)" (the speed part only when the time is
       above zero; MB/s = bytes / 1 000 000 / seconds, no decimals) + ", {n} fanns redan" when not zero +
       "."; then up to 20 "Misslyckades: {failure}" error lines and "… och {n} till"; then, when there is
       more than one folder, "Mappar: {folder} ({n}), …".
- **Rules:** no drawing; no `unwrap`/`expect`/`panic!` in non-test code; no new external crates; rules 6
  and 7.
- **Tests to add:** `cards()` answers with its id (the list itself depends on the machine); `can_start`
  in each state (and that it changes nothing); `rescan` asks for `rescan = true` and is ignored while
  running; the key functions (Ctrl+I yes; I, Ctrl+Shift+I, Alt+I no; Escape and Enter; with Ctrl
  nothing); every text function, including the zero cases, the 20-failure limit and one versus several
  folders.
- **Done when:** `.\tools\check.ps1` is all OK.
