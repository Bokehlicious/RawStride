# ADR 0017: The RawStride interface – main window, preview and inspector

- Status: **accepted by the owner (2026-11-12).** The owner approved the proposals of every open
  question (U-Q1–U-Q4, U-Q6–U-Q8, §6), the FastRawViewer additions (§2.6), the metadata window
  (§2.3.1), the order with a usable version first (§7.1) and the list of what is not included
  (§7.2). Nothing is built, installed or added as a dependency by this ADR. Each step in §7 becomes
  its own work part, with its own approval and commit.
- **Amended by the owner (2026-11-13) after a review of 32 findings:**
  - the deprecated IIM fields stay read-only (finding 1, §2.3.1);
  - one screen and **one window** first: the preview is a full-screen mode of the main window, and
    the separate preview window waits for a second screen (finding 5, §2.2, U-Q1);
  - the filmstrip moves to stage A (U3);
  - current image and selection (finding 6, §2.2);
  - the rules for previous/next in the metadata window (finding 7, §2.3.1);
  - menu names are decided in U1 (finding 21, §2.1).

  The remaining findings were clarifications, written in on the owner's go-ahead the same day.
- Date: 2026-11-12; amended 2026-11-13
- Related:
  - [GUI_REQUIREMENTS.md](../GUI_REQUIREMENTS.md) (GR1–GR13), in particular GR6 (keyboard first),
    GR7 (DPI, other screen), GR12 (imports open in a tab) and GR13 (folders open as a contact sheet);
  - [requirements-image-viewer.md](../requirements-image-viewer.md) (V1–V8, G1–G4), which is
    unchanged and applies to every view that shows an image. Its "Not covered" item (a second
    screen) stays future: stage A uses one screen (§2.2);
  - ADR 0009 (the loupe program), 0010 (import and tabs), 0011 (contact sheet), 0013 (ratings and
    labels), 0014 (filter and multi-selection), 0016 (colour management), 0007 (IPTC; D16 the modal
    metadata window on `M`);
  - [clean-room.md](../clean-room.md): this ADR describes workflow and layout in our own words.

## 1. Context

- **The owner's goal from the start** is a culling and ingest program that works the way
  professional photo-desk culling tools work: a fast contact sheet, a separate key-driven preview,
  metadata always at hand, ingest running in the background. Plus what such tools lack: a real RAW
  view next to the embedded JPEG (G4, V7).
- **That goal was studied on 2026-11-04** (levels A, B, C and D under clean-room.md; the raw notes
  are outside the repo). The study proposed an interface of its own. Only the performance part
  became requirements (V1–V8). The interface proposal was never turned into a decision.
- **What was built instead** (ADR 0009–0016 and milestone 8) follows the feature list of the first
  Tauri prototype (GR8): one window, folder tabs, a contact sheet that switches to a loupe in the
  same area, a status line, and separate dialogs for import, settings and metadata. Each part works
  and is tested, but the whole does not feel like the intended tool.
- **The core is reusable as it is:** `pw-viewcore` (views, caches, RAW and JPEG modes), `pw-thumbcache`,
  `pw-ingest`, `pw-metadata` with the safe write, `pw-colour`, the rating store, the filter and the
  selection logic. This ADR changes the **shell** around them, not the core. The one exception is
  the RAW histogram, which needs a new function in `pw-libraw` (§2.6, U7).

## 2. Decision

RawStride gets a **main window** for browsing and a **preview** for viewing (in stage A a
full-screen mode of the main window, §2.2), with the inspector and tasks always available.
Workflow: contact sheet → preview → metadata, all from the keyboard.

### 2.1 Main window

```
┌ Menu bar (menus named and ordered in U1) ──────────────────────────────────────────────────────┐
│ Command bar: Importera · Öppna mapp · Visa (alla/markerade/…) · ★ filter · färgfilter · storlek │
├──────────────┬────────────────────────────────────────────────────────────┬──────────────────┤
│ Library      │ Folder tabs: [2026_10_30 fågel ×] [RAW ×] [+]               │ Inspector        │
│ ▾ Mappar     │ Contact sheet (virtualised grid, size control)              │ ▾ Info (Exif)    │
│ ▸ Favoriter  │   cell: thumbnail · name · stars · label bar · RAW/JPEG     │ ▾ Metadata       │
│ ▸ Senaste    │   badge · reject mark                                        │ ▸ Histogram (U7) │
│ ▾ Uppgifter  │                                                              │ (collapsible)    │
│  (import,    │                                                              │                  │
│   writes)    │                                                              │                  │
├──────────────┴────────────────────────────────────────────────────────────┴──────────────────┤
│ Status: n of N selected (MB) · filter · source of the current image · task progress           │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Menu bar** with every command, each showing its key. Commands that exist today are moved into
  it (import, settings, ratings, labels, filter, RAW/JPEG mode, metadata). **The menus are named and
  ordered from RawStride's own commands in U1**, not after another program's menu bar (clean-room.md;
  owner, 2026-11-13, review finding 21).
- **Command bar** with the controls used while culling: import, **open folder** (new: Ctrl+O, which
  ADR 0009 §3.3 reserved, with the folder dialog `rfd` already used), the filter of ADR 0014
  (shown/selected, stars, labels) and thumbnail size. **Sort order** is new function and comes in
  its own step (U14), not in stage A.
- **Library (left):** a folder tree of the computer's drives, favourites, recently opened folders.
  Clicking a folder opens it in a tab (GR13 unchanged: as a contact sheet).
- **Tasks (left, below):** running and finished background work: import with per-file progress and
  a **stop** button for a running import, pending metadata and rating writes, errors as a list (not
  modal message boxes).
- **Folder tabs and contact sheet:** as today (ADR 0010, 0011); sort order is added in U14.
- **Inspector (right):** collapsible sections for the current image:
  - **Info:** an Exif summary (camera, lens, focal length, aperture, exposure time, ISO, capture
    date). This is new function: the program shows no Exif today. The source per file type is
    decided in U2 (`pw-raw` already reads lens, exposure time and ISO for RAW files).
  - **Metadata:** every value the image has, with its origin, as the `M` window shows them but
    read-only: the 22 fields of D12 and the rest, the open and decided conflicts and the notices.
    Read with the rules of T103 for a panel that follows the current image: a 200 ms wait, at most
    one read at a time, old answers discarded. With several images selected it shows the current
    image.
  - **Histogram:** comes with U7.

  Editing stays in its own window (§2.3).
- **Status bar:** selection and total, active filter, the source of the shown image (G1), progress.
- Both side columns can be resized, collapsed and hidden with one key (named in U1; not Tab, which
  changes tab today); their state is remembered (§2.5).

### 2.2 Preview

```
┌ Title line: TN6073.ARW · RAW 9504 × 6336 · 100 % · 3 of 270 · RAW | JPEG ───────────────────┐
├──────────────────────────────────────────────────────────────────────┬──────────────────────┤
│ Image area (one image; two with compare, U6)                         │ ▾ Info               │
│ overlay: name · stars · label · source                               │ ▾ Zoom               │
│                                                                      │ ▸ Histogram (U7)     │
│                                                                      │ ▸ Clipping (U7)      │
├──────────────────────────────────────────────────────────────────────┴──────────────────────┤
│ Filmstrip (all shown images, or only the selected)                                          │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Stage A: a full-screen mode of the main window (owner, 2026-11-13).** The owner uses one
  screen and cannot test two (review finding 5). Opening the preview switches the main window to
  full screen with only the preview: no frame, menu or taskbar. Escape returns to the contact sheet
  in the same tab. To the user this looks the same as a separate full-screen window on one screen,
  without the risk of a second OS window (§5).
- **A separate preview window** that can be moved to a second screen, with that screen's DPI (GR7)
  and colour profile (ADR 0016: images are converted to the profile of the screen the program
  started on, and the caches hold converted pixels), waits until the owner has a second screen to
  test with (§7.2).
- **Current image and selection (owner, 2026-11-13, review finding 6).** A step in the preview
  moves the **current image** in the preview and the contact sheet together; when the preview is
  closed, the last image shown is the current one in the contact sheet. A **selection of several
  images is not changed by a step** (this replaces ADR 0014 Q5 for the preview). With the filmstrip
  showing only the selected images, steps go only between them.
- **The loupe of today becomes the preview's image area.** V1–V8 and G1–G4 apply unchanged: zoom and
  position persist (V6), RAW and JPEG are chosen by the user (G4), the source is always stated (G1).
  The rating badge and the RAW/JPEG control (G4) stay visible in the preview whatever `I` does.
- **Right column:** Info and Zoom in stage A (U3); Histogram and clipping in U7.
- **Filmstrip** (U3, stage A): the contact sheet's thumbnails, order and filter in one row, or only
  the selected images.
- **Compare two images** side by side or above/below, with zoom and pan locked or free (U6).
- **Technical assessment tools** in the right column and on the image (§2.6).
- The right column and the filmstrip can be hidden with one key, leaving only the image.

### 2.3 Metadata

- The inspector shows the metadata of the current image read-only (§2.1). Today the values are
  only seen inside the `M` window, since D16 replaced the right-hand panel of T103; the inspector
  brings them back next to the image, without opening the window or locking the image.
- **Editing** happens in the metadata window, opened with `M` as today, extended as in §2.3.1.
- Later steps (not in this ADR): keyword panel, find and replace (§7.2).

#### 2.3.1 The metadata window: every field, and freedom to work (owner, 2026-11-12)

The window of D16 edits 22 fields (D12); every other property is shown read-only under "Övrig
metadata", and the window is locked to one image until OK or Avbryt. The owner wants the freedom of
a full photo-desk caption window.

- **What can be edited** (review finding 3): the fields of the six sections below. Every other
  property of the property table (for example image regions, artwork, products in the image, the
  AI fields, contributors, registry entries, rights expressions, data mining, maximum available
  size, model age, event id) is shown read-only under "Övrig metadata" until a later step decides
  on it. **The GPS coordinates** are read-only until the GPS ADR (§7.2).
- **Except the deprecated IIM fields (owner, 2026-11-13, review finding 1):** Prioritet, Kategori
  and Tilläggskategorier (`photoshop:Urgency`, `photoshop:Category`,
  `photoshop:SupplementalCategories`) are shown **read-only** in their section. ADR 0007 §5.5 is
  unchanged: they are kept and synchronised, never edited, created or removed. If a recipient ever
  requires them, §5.5 is revisited first.
- **Sections in a long scrolling form**, each collapsible, in this order:
  1. Innehåll: Titel, Rubrik, Beskrivning, Alternativtext, Utökad beskrivning, Beskrivningens
     författare, Nyckelord, Person i bilden, Organisation i bilden (namn, kod), Händelse;
  2. Fotograf och rättigheter: Fotograf, Fotografens titel, kontaktuppgifter, Copyright, Credit,
     Källa, Användarvillkor, Webbadress för rättigheter, Upphovsrättsskyddad;
  3. Tid och plats: Skapad; **Plats, Stad, Län/delstat, Land, Landskod** (the older IPTC Core
     fields, also written to IIM, as today; review finding 2); Plats där bilden togs (underplats,
     stad, län/delstat, land, landskod, världsdel, plats-id); Plats som visas (several, with
     "kopiera från Plats där bilden togs"). The older fields and "Plats där bilden togs" are
     separate properties and are **not synchronised** automatically;
  4. Uppdrag: Instruktioner, Jobb-id, Prioritet, Kategori, Tilläggskategorier (read-only),
     Ämneskod, Genre, Scenkod;
  5. Licens (PLUS): Bildskapare, Upphovsrättsinnehavare, Licensgivare (each up to 3), Bildleverantör,
     modell- och egendomsrelease, Digital källtyp;
  6. Betyg och etikett.

  The user chooses which sections and fields are shown and in which order (Settings), so unused
  fields take no room. The Swedish names follow D11; new names are proposed with the step. The
  PLUS fields are built in U10 together with milestone 8's I11, after the PLUS vocabulary is read
  (ADR 0007 §16.4).
- **Lists and structures** (persons, locations, creators, licensors) get one row per entry with add
  and remove buttons, within the cardinalities of ADR 0007 §8.
- **Not locked to the image.** Previous/next buttons and "spara och nästa" (also as keys) move the
  window to the next image in the contact sheet's order. With unsaved edits, moving or closing asks
  "Spara ändringarna?" (Spara / Släng / Avbryt), so nothing is lost and nothing is written
  silently. This replaces D16's lock (U-Q8, approved 2026-11-12). The rules (owner, 2026-11-13,
  review finding 7; they apply from stage A):
  - the window stays **modal**: the contact sheet and preview take no keys while it is open;
  - previous/next follow the **shown images** in the contact sheet's order (after the filter), and
    skip image types without metadata support;
  - **Escape** asks the same Spara / Släng / Avbryt question, replacing D16's "Släng ändringarna?";
  - if the **write fails**, the window stays on the image with the edits kept and the reason shown;
    with **write notices** it stays and shows them, and moves on only on the next key press.
- **Several images:** "använd på alla markerade" writes the edited fields to every selected image,
  with one confirmation that names the count. Fields not touched are left as they are on each image.
  The rules of ADR 0007 apply (review finding 19):
  - it runs as a batch under §9.5: each file is its own write, with a summary of the files that
    failed;
  - an emptied copyright field is not emptied on the other images (§8); other emptied fields are
    named in the confirmation (as D14);
  - files with open conflicts are listed (Q1).

  The same rules apply to "klistra in alla fält" on several images.
- **Speed while captioning:**
  - each field has a list of recent values and the user's own values (▼ beside the field);
  - "kopiera alla fält" and "klistra in alla fält" between images;
  - load and save a template from the window (the template format of ADR 0007 §10); which of the
    modes of §10 (keep, replace, append, clear) a template uses when loaded in the window is
    decided in U13;
  - variables in text fields (for example date, file name, camera), expanded when written; the
    variable list is decided in its step.
- **What stays read-only, and why** (rules that protect the files, ADR 0007):
  - values RawStride cannot read safely (unknown namespaces, unreadable values): shown, never
    rewritten (§8);
  - hierarchical keywords: kept unchanged until a keyword step can edit them (§8);
  - camera data (Exif such as exposure and lens): shown in the inspector's Info, not edited.

  A manufacturer RAW file is still never changed; its edits go to the sidecar as today (§4).

### 2.4 Keyboard

- One key map for the contact sheet and the preview, stored in a file and changeable in Settings
  (U8). Until then the keys are fixed in code, as today.
- RawStride defines its own default map. Common functional keys are kept (arrows, Home/End, Space,
  Z for fit/100 %, +/−, Escape, F11). The rating and label keys of ADR 0013 are unchanged.
- **Stage A** (review finding 15; today's keys of ADR 0009 and 0011 kept):

  | Action | Keys |
  |---|---|
  | Open the preview from the contact sheet | Enter, Space, G, double-click |
  | Close the preview | Escape, G, Enter |
  | Fit ↔ 100 % in the preview | Z, double-click (as ADR 0009) |
  | Hide the side columns (main window) or the right column and filmstrip (preview) | one key, named in U1; not Tab (it changes tab) |
  | Full screen for the main window | F11 (U1) |
  | Show and hide the inspector | `I` (U2) |

- No other program's complete map is copied (clean-room.md, own expression).

### 2.5 Look

- A dark, colour-neutral theme for image work; a light theme as an option (U-Q3; built in U1).
- Text contrast meets **WCAG AA** in both themes (a dark theme easily gets too little contrast).
  Font, size and spacing are chosen in U1.
- One accent colour for selection and active controls.
- **Icons:** an openly licensed icon set is a new dependency. It is chosen in its step with the
  owner's approval and a licence check (GR10, ADR 0002).
- **Windows-native window frames**, DPI-correct (GR7). The study proposed an own compact title bar;
  native frames are chosen because they keep Windows' own DPI, snapping and accessibility behaviour.
- **Remembered state** (window position, size, full screen, column widths, open sections) is saved
  in the settings file of D10 (`settings.txt`), not with eframe's `persistence` feature, which stays
  off (GUI_REQUIREMENTS §6).

### 2.6 Technical assessment in the preview (owner's choice, 2026-11-12)

The owner chose the main window and preview above as the direction, with these additions for
judging one image technically. They are drawn in the preview's right column and as overlays on the
image. None of them changes a file; they only affect what is shown. They are the owner's own wishes,
described in our own words.

- **RAW histogram and exposure statistics (U7).** In RAW mode the histogram is computed from the RAW
  data after white balance, per channel, not from the displayed 8-bit image. Below it: the share of
  pixels at the clipping level per channel, and the share in the deepest shadows. In JPEG mode the
  same panel shows the histogram of the embedded JPEG and says so (G1); it is computed **before**
  the conversion to the screen profile (ADR 0016). `pw-viewcore` only receives finished 8-bit RGB
  images; the unpacked RAW data stays in the LibRaw session (`pwlr.cpp` unpacks once). So this
  needs **a new function in the C++ shim and `pw-libraw`** that computes the statistics from the
  already unpacked data; no new decode [T, confirmed in U7]. The histogram section of the
  inspector comes in the same step.
- **Clipping overlay (U7).** Highlights at the clipping level and shadows below a threshold are
  marked on the image, each with its own key, in RAW mode from the RAW data and in JPEG mode from
  the JPEG. In RAW mode the RAW positions are mapped to the shown image (orientation, borders, half
  size).
- **Focus marking (U9).** Edges with high local contrast are marked in a colour on the image (one
  key), so sharp areas can be seen at fit without zooming. Computed from the shown image.
- **Move on reject or select (U9; setting, off by default).** When the setting is on, a reject
  moves the image with all its companion files (below) to a subfolder `_rejected`, and a select key
  moves them to `_picked`, in the image's own folder. Names and keys are decided in U9. This answers
  ADR 0013 §13 (review finding 8):
  - **both** happen: the rating is set and the files are moved;
  - a pending rating or metadata write to the file or its sidecar is **finished before** the move,
    since the write journal (ADR 0007 §9.5) points at the path;
  - the image **leaves the tab's list**; the contact sheet closes the gap;
  - the rating store, the metadata cache and the thumbnail cache **follow the files** to the new
    path;
  - **undo** (Ctrl+Z, U-Q6) moves the files back and restores the rating; changing the rating of a
    moved image back is done through undo, since the image is no longer in the folder's list.
- **Companion files move with the image (owner, 2026-11-12).** Every file in the same folder that
  belongs to the image moves with it, so no other program loses its edits:
  - the RAW+JPEG partner (same base name);
  - XMP sidecars, both `<base>.xmp` and `<base>.<ext>.xmp` (the second kind is never read or written
    by RawStride, ADR 0007 Q8, but it is moved);
  - other programs' edit files, for example DxO PhotoLab `.dop`, RawTherapee `.pp3` and ON1 `.on1`,
    named `<base>.<ext>.<suffix>` or `<base>.<suffix>`.

  The suffixes are a list in Settings with these as the default. The naming of each program's files
  is checked on real files before U9 (the owner's DxO PhotoLab files first). The move is all or
  nothing: if one file cannot be moved (open in another program, name taken in the subfolder), the
  files already moved are moved back and the reason is shown. Edit data kept outside the image's
  folder (for example a Capture One session or a Lightroom catalogue) is not moved; such a program
  must find the file again itself.

Not included: temporary exposure or white balance correction for assessment (§7.2).

### 2.7 Own design

The ideas of the study are free to use, but the form is our own (clean-room.md). Order, texts and
placement of titles, columns and buttons are decided in each step from RawStride's own commands,
without the study notes as a template.

## 3. What stays

- The whole core (§1) and every behaviour requirement V1–V8, G1–G4, GR1–GR13.
- Folder tabs, the contact sheet, ratings, labels, filter, multi-selection, the import engine, the
  safe metadata write, colour management.
- The Tauri app is not touched (D7). Its removal is a separate decision (GR8; V3, V4, V7).

## 4. What changes in the existing program

| Today | Proposed |
|---|---|
| One window; the loupe replaces the contact sheet in the same area, inside the window frame | Main window with library, contact sheet and inspector; the preview as a full-screen mode with right column and filmstrip (a separate window later, §2.2) |
| No menu bar; commands only by key | Menu bar and command bar, keys shown in the menus; "Öppna mapp" (Ctrl+O) |
| `I` hides the status line in the loupe (with the rating badge and the RAW/JPEG control); no info panel (T103's panel was removed by D16) | `I` shows and hides the inspector; the rating badge and RAW/JPEG control always visible in the preview |
| No Exif shown | Inspector Info with an Exif summary |
| Import progress in the status line | Tasks area with per-file progress, a stop button and an error list |
| Folders opened by drag-and-drop, import or the command line | Folder dialog, folder tree, favourites and recent folders as well |
| Metadata window: 22 editable fields, the rest read-only; locked to one image until OK or Avbryt | The fields of six sections editable, in sections the user can arrange; previous/next and "spara och nästa"; recent values, copy/paste of all fields, templates, variables; "använd på alla markerade" (§2.3.1) |
| No compare, filmstrip, histogram or sorting | Filmstrip (stage A); two-image compare, RAW histogram with exposure statistics, clipping overlay, focus marking, sort order |
| A reject only writes the rating | Optionally also moves the image and its companion files (sidecars, RAW+JPEG partner, `.dop` and other edit files) to `_rejected` (select: `_picked`) |
| Fixed keys in code | One key map in a file, editable in Settings |

## 5. Risks and things to measure

- **A second OS window in egui with glow [T]** is avoided in stage A (§2.2). eframe 0.37's glow
  integration has multi-viewport support (one shared painter for all viewports, one OpenGL context
  made current and one buffer swap per window). When the separate window is taken up, a prototype
  (U0) must show that it meets V1–V8, including the time per step when both windows are redrawn
  (each swap may wait for the screen refresh), the choice between an immediate and a deferred
  viewport (how the program state is shared), and the second screen's DPI (GR7) and colour profile
  (ADR 0016).
- **Texture memory** when the contact sheet's thumbnails and the preview are both held: the separate
  texture budget (GUI_REQUIREMENTS §7) may need a new share.
- **The filmstrip** must not slow down stepping: its thumbnails are fetched below the shown image's
  priority, and U3 measures this with `latency.ps1`.
- **Focus** (GR6): keys must go to the view the user looks at, and never be lost after closing a
  dialog; with a separate window later, also between the two windows.

## 6. Questions (all answered by the owner, 2026-11-12: every proposal approved)

| # | Question | Answer (owner, 2026-11-12: as proposed) |
|---|---|---|
| U-Q1 | Should the preview also be usable inside the main window (for a one-screen laptop), or only as its own window? | Both: its own window by default; "preview in main window" as a setting. **Changed 2026-11-13 (owner):** stage A has only the full-screen mode in the main window; the separate window comes with a second screen (§2.2) |
| U-Q2 | Which key opens the preview from the contact sheet? | Space and double-click; Enter does the same. G is kept as today (§2.4) |
| U-Q3 | Dark theme by default? | Yes, with a light theme in Settings |
| U-Q4 | Order against the rest of milestone 8 (I11–I13) | Finish I12 and I13 first; I13 documents milestone 8 without the PLUS fields, which move with I11 to U10 (§7.1) |
| U-Q5 | — | Removed during the drafting (2026-11-12) |
| U-Q6 | Move on reject/select: what happens to a file already moved when its rating is changed back? | It is moved back to the image's folder; a move can be undone with Ctrl+Z (§2.6) |
| U-Q7 | Focus marking colour and strength | One colour (configurable later), one strength; tuned on the owner's test files in U9 |
| U-Q8 | D16 locked the window to one image until OK or Avbryt. May the window instead move between images, asking about unsaved edits? | Yes: previous/next and "spara och nästa", with Spara / Släng / Avbryt when there are unsaved edits; rules in §2.3.1 |

## 7. Steps

Each step ends with tests (GR11), the owner's hand check and, where an image is shown, `latency.ps1`.

| Step | Content |
|---|---|
| **U0** | *Postponed (owner, 2026-11-13):* prototype of the separate preview window in a second egui viewport (glow); V1, V3, V6 and the step time with both windows redrawn measured; immediate or deferred viewport; focus between the windows. Done when the separate window is taken up (§7.2) |
| **U1** | Shell: menu bar (own names, §2.1), command bar with "Öppna mapp" (Ctrl+O), status bar, resizable and collapsible side columns with the hide key, F11; dark and light theme (§2.5); state remembered in `settings.txt`; existing commands moved into the menus. No sorting |
| **U2** | Inspector: Info (Exif summary, new; fields and sources §2.1) and Metadata (every value read-only, read with T103's rules, §2.1); `I` shows and hides the inspector; sections remembered. The `M` window is unchanged apart from previous/next (§7.1) |
| **U3** | Preview as a full-screen mode of the main window (§2.2): the loupe moved there; open and close keys (§2.4); the right column (Info, Zoom); the rating badge and RAW/JPEG control always visible; current image linked with the contact sheet and the selection kept (finding 6); the **filmstrip** (moved from U6, owner 2026-11-13): the contact sheet's thumbnails, order and filter in one row, or only the selected, scrolled to the current image, click to jump, hidden with the hide key; its thumbnails are fetched below the shown image's priority so stepping (V1–V8) is not slowed |
| **U4** | Library: folder tree, favourites, recent folders |
| **U5** | Tasks area: import progress per file, stop button, pending writes, error list |
| **U6** | Compare two images (side by side, above/below, locked zoom); the filmstrip moved to U3 (owner, 2026-11-13) |
| **U7** | RAW histogram and exposure statistics (new `pw-libraw` function), in the preview and the inspector; clipping overlay (§2.6) |
| **U8** | Key map file and its page in Settings |
| **U9** | Focus marking; move on reject or select as a setting, with undo (§2.6); companion file names checked on real files first |
| **U10** | Metadata window: every simple text and date field of the six sections editable, with the PLUS fields (merged with I11; the PLUS vocabulary read first, ADR 0007 §16.4); sections and the user's choice of fields (§2.3.1) |
| **U11** | Metadata window: lists and structures (persons, locations, creators, licensors, releases) with add and remove |
| **U12** | Metadata window: previous/next and "spara och nästa" (the first part in stage A), recent values, copy/paste of all fields, "använd på alla markerade" with the batch rules (§2.3.1) |
| **U13** | Metadata window: templates from the window (with the modes of ADR 0007 §10) and variables in text fields |
| **U14** | Sort order in the command bar and the contact sheet |

### 7.1 Order: a usable version first (owner, 2026-11-12)

The owner wants a **usable version early**, to judge from real use which parts should move forward
or back. So the steps are not done in number order:

1. **Stage A – the usable version.** U1, U2 and U3, plus previous/next in today's `M` window (the
   first part of U12, with the rules of §2.3.1). U3 includes the filmstrip (owner, 2026-11-13:
   important; the two-image compare waits in U6). It moves today's features into the new shell
   (menu bar, side columns, inspector, full-screen preview) and adds as little new function as
   possible. Then the owner uses it on real work.
2. **The owner reorders** the remaining steps from that use. This list is only the starting order.
3. **Stage B (proposed order):** U10 and the rest of U12 (the metadata window's freedom, merged with
   milestone 8's I11 so the PLUS fields are built once), U5, U4, U14, U6, U7, U11, U13, U9, U8.

U0 is not part of stage A: the preview is a mode of the main window (owner, 2026-11-13), and the
separate window waits (§7.2).

### 7.2 Not in this ADR (known wishes, kept so they are not forgotten)

| Wish | When (owner, 2026-11-12) | Note |
|---|---|---|
| **GPS**: import a GPX track and match it to capture times (with a time offset), set a position on a map, fill in city, country and location from the position (reverse geocoding) | **Later: the next metadata ADR after this one** | Needs new dependencies (a map view, probably an online place service): an owner decision and a licence check (ADR 0002) |
| **Find and replace** in metadata across many images (fields, case, whole words, file names) | Further out | Uses the safe write of ADR 0007 §9 |
| **Keyword panel**: a flat list of approved keywords, and structured keywords (hierarchy, synonyms, import/export as text) | Further out | Lets RawStride edit the hierarchical keywords it only keeps today (ADR 0007 §8) |
| Tagging (a yes/no mark apart from stars and labels) | Later, after stage A (owner, 2026-11-12) | |
| Loupe under the mouse pointer in the contact sheet (100 % while a key or button is held) | Later, after stage A (owner, 2026-11-12) | |
| Copy, move and rename tools for selected images | Later, after stage A (owner, 2026-11-12) | |
| Chosen fields as text lines under each thumbnail | Later, after stage A (owner, 2026-11-12) | The thumbnail shows the file name until then |
| Temporary exposure and white balance for assessment | Later, after stage A (owner, 2026-11-12) | Needs RAW processing options the view core does not have today |
| The separate preview window (U0), movable to a second screen with that screen's DPI and colour profile | When the owner has a second screen to test with (owner, 2026-11-12 and 2026-11-13) | Review finding 5: colour conversion is per start screen today (ADR 0016) |
| Code replacements (short codes expanded to text while captioning) | Not yet placed (review, 2026-11-13) | From the study's list of missing functions |
| A catalogue across folders (`pw-catalog`) | Not yet placed (review, 2026-11-13) | From the study's list of missing functions |
| Floating, freely placed panels | Future requirement (requirements-image-viewer.md, "Not covered") | |

## 8. Consequences

- RawStride gets the intended workflow without changing the tested core (apart from the RAW
  histogram function, U7).
- ADR 0009–0011 and D16 keep their behaviour rules; their layout parts were replaced by this ADR
  when it was accepted. ADR 0014 Q5 is replaced for the preview (§2.2), and D16's lock and Escape
  question for the metadata window (§2.3.1).
- `docs/architecture.md` and `docs/README.md` were updated when this ADR was accepted.
