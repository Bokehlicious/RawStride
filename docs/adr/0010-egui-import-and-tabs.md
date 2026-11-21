# ADR 0010: Milestone 2 – import and folder tabs in the egui program

- Status: **accepted by the owner (2026-11-08, step M0).** The four points of §2 and the answers to
  the questions of §11 (the proposals, accepted as written) are decided. Nothing is built, installed
  or added as a dependency by this ADR. Each step in §9 is its own work part, with its own approval
  and commit.
- **Progress:** **all steps done (2026-11-09).** M1 and M2 on 2026-11-08; the contact sheet (ADR 0011)
  was built next; M3–M8 on 2026-11-09. Step A was accepted by the owner with a real card (M6); step B
  (backup, job and number, IPTC template, eject) was checked by hand part by part (M7).
- Date: 2026-11-08
- Related:
  - [GUI_REQUIREMENTS.md](../GUI_REQUIREMENTS.md) (GR1–GR11);
  - ADR 0009 (milestone 1, the loupe program; §3.2 lists the import as a later milestone, §12 says
    later milestones get their own ADRs);
  - ADR 0005 (ingest engine and today's Tauri dialog);
  - ADR 0007 (metadata, proposed), for the IPTC template in step 2;
  - `docs/tasks/done/` (owner requirement of 2026-11-08: the imported folder opens automatically in
    a new tab).

## 1. Context

- The egui program `pw-loupe` (milestone 1) shows one folder at a time, opened from the command line
  or by drag-and-drop. It has no dialogs and no tabs (ADR 0009 §3, §11 decisions 2 and 3).
- Import (ingest) is half of the product's purpose (ADR 0005). Today it exists only in the Tauri app
  (`src-tauri/src/ingest.rs`, `ui/web/ingest.js`). GR8 requires it in the egui GUI before the switch.
- The import engine is already shared Rust: `pw-ingest` (scan, plan, verified copy, cancel) and, since
  2026-11-08, the path checks (`resolve`, `inside`), the drive helpers (`drive_of`, `eject`) and the
  folder summary (`folder_summary`). The Tauri file keeps only glue, request validation and card
  detection (`ingest_volumes`).
- After an import, the owner wants the imported folder to open **automatically in a new tab**, as in
  the workflow the owner is used to. The Tauri app only offers an "Öppna i visaren" button.

## 2. Decisions fixed by the owner (2026-11-08)

1. **The import lives in `pw-loupe`.** The egui program grows from the loupe into the main program.
   The program and window may be renamed later; that is not part of this ADR.
2. **Simple folder tabs now.** A tab bar where each tab is one open folder. The import opens the
   imported folder in a new tab.
3. **The Windows folder dialog** is used to choose source, destination and backup, through a small
   helper crate. The crate and version need their own approval with a licence check (GR10).
4. **Two steps.**
   - **Step A:** source (with automatic card detection), destination, folder and file-name templates,
     plan preview, verified copy, progress, cancel, summary, and opening the result in a new tab.
   - **Step B:** backup destination, IPTC template, job name and running number, and ejecting the card.

## 3. Scope

### 3.1 Step A

| Area | Content | Basis |
|---|---|---|
| Open the dialog | A control in the window and **Ctrl+I** (the key the Tauri app uses) | ADR 0005 |
| Source | Detected cards (drives with `DCIM`) offered automatically; any folder through the folder dialog or drag-and-drop. **Owner, 2026-11-16 (T161):** while the dialog is open and no import runs, the cards are looked for again every 2 s, so a card put in or taken out shows at once | ADR 0005 |
| Destination | Folder dialog, typed path or drag-and-drop | – |
| Templates | Folder and file name, with the variables of ADR 0005 point 4 and the default templates of ADR 0005 choice 1. Template errors mark the field | ADR 0005 |
| Validation | Absolute paths, no `..`, destination not on or above the source; the same rules and messages as today, shared through `pw-ingest` (§4.3) | – |
| Preview | Counts, bytes, conflicts, files without EXIF time and the first lines of the plan; recomputed shortly after each change; the scan is reused while only templates change and is redone before copying | ADR 0005; fix of 2026-11-08 |
| Copy | `pw-ingest::execute` on a background thread; the UI only polls progress (G2, GR2) | ADR 0005 point 6 |
| Progress and cancel | Files, bytes and current file; cancel stops between chunks and leaves no final-named file | ADR 0005 point 6 |
| Summary | Copied, already present, failed (with reasons), time and speed, destination folders | – |
| Open the result | When the copy is complete, **every destination folder that received a copied file** opens **in its own tab** automatically, oldest images first and the newest folder's tab active (owner, 2026-11-16, T163; until then only the folder with the most files opened) (§11, Q2 for cancel and failures), shown as a **contact sheet** (ADR 0011 §2, GR13; added 2026-11-09). **Changed by ADR 0026 §3.4 (step F4, 2026-11-21; until F4 is done, as above):** each folder's tab opens when its first image is copied, verified and given the template, and fills as the import goes on; the first folder's tab is active, the newest becomes active at the end only if the user has not moved (F-Q10) | Owner requirement |
| Settings | Remembered for the running session only. A settings file comes later (§11, Q5) | ADR 0009 decision 3 |

### 3.2 Step B

- **Backup destination**, checked against the source and the destination as today.
- **IPTC template** (creator, copyright, credit, e-mail, telephone, web, title, description), written to
  the new copies after a complete copy, through `pw-metadata`'s worker (not on the UI thread).
  **Changed by ADR 0026 §3.4 (F-Q11, step F4, 2026-11-21; until F4 is done, as above):** written
  to each shot's images right after the shot's files are copied and verified, by the import
  worker's own ExifTool with the safe write (ADR 0007 §9.5), before the images are shown.
- **Job name** and **first running number**.
- **Eject the card** after a complete import, with `pw-ingest::eject`.

### 3.3 Folder tabs (both steps)

- A tab bar above the image. Each tab is one folder: its path, its file list and the current image.
- **One view core for all tabs.** Only the active tab reads and prepares images; the 4 GB core budget
  and the texture budget are not split. Switching tabs opens that tab's folder in the core again and
  goes to its remembered image.
- A folder dropped on the window opens **in a new tab**; an empty active tab is filled instead, and a
  folder that is already open is activated. A folder from the command line opens in the first tab
  (§11, Q3, changed 2026-11-08).
- Keys (§11, Q4): **Ctrl+Tab** / **Ctrl+Shift+Tab** next / previous tab, **Ctrl+W** close tab.
  Clicking a tab activates it. The last tab cannot be closed; it becomes empty instead.
- If the imported folder is already open in a tab, that tab is activated and reloaded instead of
  opening a second tab.

### 3.4 Out of scope

- Ratings and labels in the egui program (a later milestone). The contact sheet was built first, in
  ADR 0011 (done 2026-11-09).
- A settings window and a settings file (§11, Q5).
- Writing IPTC through the safe write procedure of ADR 0007 §9 (it applies once ADR 0007 is accepted;
  step B uses today's `write_iptc`, as the Tauri app does) (done: ADR 0007 I9, T91, 2026-11-12).
- Changes to the Tauri app's behaviour. It keeps working until the switch (GR8).
- Opening several tabs at start from the command line.

## 4. Architecture

### 4.1 Where the code goes

| Part | Crate | Notes |
|---|---|---|
| Card detection (`ingest_volumes`) | `pw-ingest` | Moved from `src-tauri`; the Tauri app calls it |
| Request validation (`IngestRequest::options` today) | `pw-ingest` | Moved from `src-tauri` as a plain function with the same messages; the Tauri app calls it |
| Tab model | `pw-loupe` (library) | Plain Rust: open, activate, close, remember the image; no drawing (GR11) |
| Import dialog state | `pw-loupe` (library) | Plain Rust state machine (§4.2); no drawing (GR11) |
| Import worker | `pw-loupe` (library) | Background thread with a bounded channel, as `MetadataWorker` does |
| Drawing | `pw-loupe` (binary) | egui window for the dialog, the tab bar, the folder-dialog buttons |
| Folder dialog | helper crate | Only after approval (§6) |

### 4.2 The import dialog as a state machine

`Editing` → (`Previewing` while a scan or plan runs) → `Editing` with a plan → `Running { progress }` →
`Done { summary }` → back to `Editing`.

- Every transition is a plain function that can be tested without a window (GR11).
- The scan, the plan and the copy run on the worker thread. The UI sends requests and polls results;
  results from an older request are ignored, as in the loupe's metadata cache.
- If the worker disconnects, a running import becomes an error, never "running" forever.

### 4.3 Shared with the Tauri app

The validation, card detection, path checks and summary are moved to `pw-ingest` first, so both
dialogs give the same answers. The Tauri app keeps its own Tauri glue only.

## 5. Behaviour requirements

- **G2 / GR2:** the window stays responsive during scan, plan and copy. No file I/O on the UI thread.
- **GR6:** the dialog works from the keyboard (Tab between fields, Enter starts when valid, Escape
  closes it when no import is running). Escape in the loupe itself stays reserved for the contact
  sheet (ADR 0009 §3.3). Focus returns to the image after the dialog closes.
- **GR11:** dialog state, validation, the tab model and the worker protocol have `cargo test` tests.
- **ADR 0005 guarantees stay:** never overwrite, verified copies, the source opened read-only, no
  final-named file after a failure or cancel.
- **Tab switching** has no requirement in V1–V8. It is measured in acceptance and reported, not judged
  (§8).

## 6. Dependencies

- **Folder dialog:** **`rfd` 0.17.2 (MIT), approved by the owner on 2026-11-09 (step M3)**, pinned as
  `=0.17.2` with `default-features = false`, in `pw-loupe` only. The licence check of the new tree for
  Windows x64 is in GUI_REQUIREMENTS §6.1: its Windows dependencies (`windows-sys` 0.61,
  `raw-window-handle` 0.6, `log` 0.4) are already in the workspace, and so is `rfd` 0.17.2 itself
  (through the Tauri app's `tauri-plugin-dialog`), so no new crate is added. It
  is added in step M5, where the dialog is drawn.
- No other new crates are expected. `pw-loupe` gains a dependency on `pw-ingest` (internal).

## 7. Coexistence with the Tauri app (GR8)

- Moving shared code to `pw-ingest` changes only where the Tauri app's functions live, not what they
  do; its tests stay green.
- `ui/web` and the Tauri dialog are not changed.

## 8. Testing and acceptance

### 8.1 Automated (`cargo test`, `cargo clippy`)

- `pw-ingest`: card detection (with a temporary folder standing in for a drive where possible) and the
  moved validation, with today's messages.
- `pw-loupe`: the tab model; the dialog state machine (valid and invalid input, preview, run, cancel,
  done, worker disconnect); the worker on temporary folders with the synthetic card of
  `pw-ingest/tests/ingest.rs`.

### 8.2 By hand (owner)

- Import from a real card into an empty folder: progress, summary, the folder opens in a new tab.
- Cancel during the copy: no final-named partial files; the result stays in the dialog.
- The window stays responsive during the copy (pan and stepping in the open tab).
- Tab switching: time until the image is shown, measured and reported.
- Step B: backup, IPTC template, eject, as tested in the Tauri app on 2026-11-08.

## 9. Work steps

| Step | Content | Result |
|---|---|---|
| **M0** | Accept this ADR; answer §11; add the auto-open requirement to GUI_REQUIREMENTS (GR12) | **Done 2026-11-08:** documentation commit |
| **M1** | `pw-ingest`: move card detection and request validation from `src-tauri`; the Tauri app calls them | Code + tests |
| **M2** | `pw-loupe`: tab model (plain Rust) and tab bar with keys | Code + tests |
| **M3** | Approve the folder-dialog crate (version, features, licence tree) | **Done 2026-11-09:** `rfd` =0.17.2 (§6) |
| **M4** | `pw-loupe`: import dialog state machine and worker (plain Rust) | **Done 2026-11-09** (T30–T33) |
| **M5** | `pw-loupe`: draw the dialog, Ctrl+I, folder dialog, open the result in a new tab | **Done 2026-11-09** (T34–T36) |
| **M6** | Acceptance by hand, step A (§8.2) | **Done 2026-11-09:** owner imported from a real card: progress, summary, the folder opens in a new tab as a contact sheet; cancel leaves no partial files; "Dölj" keeps the grid and loupe smooth during the copy; a second import shows the files as present and opens no tab |
| **M7** | Step B: backup, job and running number, IPTC template, eject | **Done 2026-11-09** (T37–T40, each checked by hand) |
| **M8** | Documentation: ADR 0005 dialog section, architecture.md, test inventory | **Done 2026-11-09:** documentation commit |

Steps M1, M2, M4 and M7 are suited to the agent task list (`docs/tasks/done/`): M1 is T20, M2
is T21, M4 is T30–T33, M5 is T34–T36 and M7 is T37–T40.

## 10. Risks

| Risk | Effect | Mitigation |
|---|---|---|
| Re-opening a folder in the core on every tab switch | A visible pause before the image | Measured in M6; caching per tab only if the owner asks (it would split the budget) |
| A native dialog blocks the UI thread while open | Other drawing pauses while the dialog is open | Acceptable for a modal choice; checked in M6 |
| Two dialogs (Tauri and egui) drift apart | Different answers for the same input | Shared validation and checks in `pw-ingest` (M1) |
| The import and the open tab use the same card | A card cannot be ejected while a tab reads from it (step B) | A tab showing the card is emptied when an import with eject starts (`Tabs::clear_inside`); checked by hand in M7 |

## 11. Questions and the owner's answers (2026-11-08)

The owner accepted every proposal below as written.

| # | Question | Decision (the proposal, accepted) |
|---|---|---|
| **Q1** | What does a tab remember? | Folder, file list and current image. Zoom and pan reset to fit when the tab is shown again |
| **Q2** | Does a cancelled or partly failed import also open a new tab? | Yes for cancelled or partly failed imports that copied at least one file, with the summary saying so; no tab when nothing was copied |
| **Q3** | Where does drag-and-drop or the command line open a folder? | **Changed by the owner after testing (2026-11-08):** a dropped folder always opens in a new tab (an empty active tab is filled instead; an already open folder is activated). The command line opens in the first tab. The first answer ("in the active tab; Ctrl+drop for a new tab") was dropped: Windows does not report Ctrl reliably while dragging from Explorer, and the owner prefers a new tab |
| **Q4** | Tab keys | Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+W, as listed in §3.3. None of them is used by the loupe today |
| **Q5** | Settings (destination, templates) between sessions | Session only in step A; a settings file is a separate decision |
| **Q6** | Maximum number of tabs | No hard limit; inactive tabs hold only paths, so they cost little memory |

## 12. Consequences

- The egui program becomes the main program, with tabs and the import; GR8 moves one feature closer.
- Import logic shared by both GUIs lives in `pw-ingest`, so the Tauri file shrinks further.
- One new helper crate (the folder dialog), after its own approval.
- The loupe's keys of ADR 0009 §3.3 are unchanged; Ctrl+I and the tab keys are added.
